//! Transaction adapter for the pure transfer reducer. No provider or file I/O
//! runs under the writer lock. Native integration calls reserve/receive with its
//! exact ticket; it may send only after reserve returns a committed decision.
pub use crate::account_transfer::TransferDispatch;
use crate::account_transfer::{
    Action, Command, Decision, EXECUTOR_REQUIRED, Snapshot, Ticket, TransferError,
};
use serde::Serialize;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use studio_runtime_store::{
    AccountTransferRepository as Repository, AgentId, AgentRepository, RuntimeStore, Transaction,
};

use studio_runtime_schema::queue_command::Field;
use studio_runtime_store::transfer_record::TransferRecord;

/// The operation named by a nonterminal, readable agent summary.
pub(crate) fn active_summary_id(agent: &studio_runtime_store::AgentRecord) -> Option<&str> {
    let studio_runtime_store::transfer_record::AgentTransferState::Summary(summary) =
        agent.account_transfer.value()?
    else {
        return None;
    };
    if matches!(
        summary.status,
        studio_runtime_store::transfer_record::TransferStatus::Completed
            | studio_runtime_store::transfer_record::TransferStatus::Cancelled
    ) {
        return None;
    }
    Some(summary.id.as_str())
}

pub fn load(tx: &Transaction<'_>, id: &str) -> Result<Snapshot, TransferError> {
    if !Repository::available(tx)? {
        return Err(TransferError::Requires(
            "store-bootstrap/account-transfer-schema",
        ));
    }
    let (op, raw) = Repository::get_typed_with_raw(tx, id)?
        .ok_or(TransferError::Refused("Unknown account transfer"))?;
    let mut ids = op
        .members
        .0
        .iter()
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    let lead = op.lead_id.as_str();
    if !ids.iter().any(|id| id == lead) {
        ids.push(lead.into());
    }
    let agents = ids.into_iter().map(|id| {
        let raw =
            Repository::agent(tx, &id)?.ok_or(TransferError::Refused("Unknown transfer member"))?;
        Ok((id, raw))
    });
    Snapshot::read_typed(op, raw.len(), agents)
}

/// A lead summary mirrors its transfer row. An active summary without the row
/// can never advance: a move to another server copies the agent record, not the
/// source server's transfer row. Remove that summary so it neither holds the
/// agent's turns nor blocks a new transfer. Returns the changed agent.
fn drop_orphan_summary_in(
    tx: &Transaction<'_>,
    agent_id: &str,
) -> Result<Option<studio_runtime_store::AgentRecord>, TransferError> {
    if !Repository::available(tx)? {
        return Ok(None);
    }
    let Some(mut agent) = AgentRepository::get(tx, &AgentId::from(agent_id))? else {
        return Ok(None);
    };
    let Some(operation) = active_summary_id(&agent) else {
        return Ok(None);
    };
    if Repository::get_typed_with_raw(tx, operation)?.is_some() {
        return Ok(None);
    }
    agent.account_transfer = studio_runtime_schema::queue_command::Field::Missing;
    AgentRepository::put(tx, &agent)?;
    let value = agent.value();
    crate::agents_create::publish_agent(tx, &value)?;
    Ok(Some(agent))
}

pub async fn drop_orphan_summary(
    store: &RuntimeStore,
    agent_id: &str,
) -> Result<Option<studio_runtime_store::AgentRecord>, TransferError> {
    let agent_id = agent_id.to_owned();
    store
        .transaction_with_error(move |tx| drop_orphan_summary_in(tx, &agent_id))
        .await
}

/// Release a member that was busy at its transfer request once its turn has
/// ended (`Snapshot::release_waiting`). The caller has not claimed the agent's
/// queued input, so the destination account's worker claims it. Returns whether
/// the member was released.
pub async fn release_waiting(
    store: &RuntimeStore,
    agent_id: &str,
    now: f64,
) -> Result<bool, TransferError> {
    let agent_id = agent_id.to_owned();
    store
        .transaction_with_error(move |tx| release_waiting_in(tx, &agent_id, now))
        .await
}

/// Release in the terminal transaction, after source-account continuation policy.
pub(crate) fn release_waiting_in(
    tx: &Transaction<'_>,
    agent_id: &str,
    now: f64,
) -> Result<bool, TransferError> {
    if !Repository::available(tx)? {
        return Ok(false);
    }
    let Some(agent) = AgentRepository::get(tx, &AgentId::from(agent_id))? else {
        return Ok(false);
    };
    let Some(operation) = agent
        .account_transfer_id
        .value()
        .map(|id| id.as_str())
        .filter(|_| agent.lazy_account_transfer.value().is_none())
    else {
        return Ok(false);
    };
    if Repository::get_typed_with_raw(tx, operation)?.is_none() {
        return Ok(false);
    }
    let Some(decision) = load(tx, operation)?.release_waiting(agent_id)? else {
        return Ok(false);
    };
    persist(tx, decision, now)?;
    Ok(true)
}

/// A transfer may bypass only its own native-start admission marker, and only
/// while its exact member is durably in the Reading phase.
pub fn owns_reading_member(
    tx: &Transaction<'_>,
    transfer_id: &str,
    member_id: &str,
) -> Result<bool, TransferError> {
    let snapshot = load(tx, transfer_id)?;
    use studio_runtime_store::transfer_record::{TransferPhase, TransferStatus};
    let operation = snapshot.record();
    if operation.status != TransferStatus::Pending
        || operation.members.get(member_id).map(|member| member.phase)
            != Some(TransferPhase::Reading)
    {
        return Ok(false);
    }
    let agent = snapshot.agent_record(member_id)?;
    Ok(agent
        .account_transfer_id
        .value()
        .is_some_and(|id| id.as_str() == transfer_id)
        && agent
            .lazy_account_transfer
            .value()
            .and_then(|state| state.pending())
            .is_some_and(|lazy| lazy.id.as_str() == transfer_id))
}

/// The account worker calls this once during startup before admitting turns.
pub async fn recover(store: &RuntimeStore, now: f64) -> Result<usize, TransferError> {
    store
        .transaction_with_error(move |tx| {
            if !Repository::available(tx)? {
                return Ok(0);
            }
            let mut changed = 0;
            for id in Repository::recovery_ids(tx)? {
                let snapshot = load(tx, &id)?;
                let decision = snapshot.decide(Command::Restart)?;
                changed += usize::from(decision.changed);
                persist(tx, decision, now)?;
            }
            Ok(changed)
        })
        .await
}
pub async fn action(
    store: &RuntimeStore,
    id: &str,
    action: Action,
    now: f64,
) -> Result<Value, TransferError> {
    let id = id.to_owned();
    store
        .transaction_with_error(move |tx| {
            let snapshot = load(tx, &id)?;
            persist(tx, snapshot.decide(Command::Action(action))?, now)
        })
        .await
}

fn replay_in(
    tx: &Transaction<'_>,
    id: &str,
    lead: &str,
    target: &str,
    scope: &str,
) -> Result<Option<Value>, TransferError> {
    let Some(key) = Repository::request_operation(tx, id)? else {
        return Ok(None);
    };
    let snapshot = load(tx, &key)?;
    if snapshot.replay(id, lead, target, scope)? {
        use studio_runtime_schema::queue_command::Field;
        let response = snapshot
            .record()
            .requests
            .value()
            .and_then(|requests| requests.get(id))
            .map(|request| &request.saved_response);
        let saved = match response {
            None | Some(Field::Missing) => {
                return Err(TransferError::Refused(
                    "The original transfer response is unavailable",
                ));
            }
            Some(Field::Null) => {
                return Err(TransferError::Refused(
                    "The original transfer response is invalid",
                ));
            }
            Some(Field::Value(saved)) => saved,
        };
        return saved
            .serialize(serde_json::value::Serializer)
            .map(Some)
            .map_err(|_| TransferError::Refused("The original transfer response is invalid"));
    }
    Err(TransferError::Refused(
        "This transfer id has different content",
    ))
}

fn excludes_account(agent: &studio_runtime_store::AgentRecord, target: &str) -> bool {
    account_is_excluded(agent.extra.get("excludedAccountKeys"), target)
}

/// The single compatibility accessor for per-chat account exclusions.
pub(crate) fn account_is_excluded(exclusions: Option<&Value>, target: &str) -> bool {
    exclusions
        .and_then(Value::as_array)
        .is_some_and(|keys| keys.iter().any(|key| key.as_str() == Some(target)))
}

/// Return a retained request before consulting mutable account eligibility.
pub async fn replay(
    store: &RuntimeStore,
    id: &str,
    lead: &str,
    target: &str,
    scope: &str,
) -> Result<Option<Value>, TransferError> {
    let (id, lead, target, scope) = (
        id.to_owned(),
        lead.to_owned(),
        target.to_owned(),
        scope.to_owned(),
    );
    store
        .transaction_with_error(move |tx| replay_in(tx, &id, &lead, &target, &scope))
        .await
}
/// No provider submission is repeated by this method. It only replays an exact
/// durable request; a genuinely new request needs the executor before any write.
pub async fn request(
    store: &RuntimeStore,
    id: &str,
    lead: &str,
    target: &str,
    scope: &str,
    target_provider: &str,
    now: f64,
) -> Result<Value, TransferError> {
    uuid::Uuid::parse_str(id)
        .map_err(|_| TransferError::Refused("Supply a transfer request id"))?;
    let id = id.to_owned();
    let lead = lead.to_owned();
    let target = target.to_owned();
    let scope = scope.to_owned();
    let target_provider = target_provider.to_owned();
    store
        .transaction_with_error(move |tx| {
            if !Repository::available(tx)? {
                return Err(TransferError::Requires(EXECUTOR_REQUIRED));
            }
            if let Some(result) = replay_in(tx, &id, &lead, &target, &scope)? {
                return Ok(result);
            }
            if scope != "team" {
                return Err(TransferError::Refused(
                    "Choose a valid transfer scope and destination provider",
                ));
            }
            let target_provider = target_provider
                .parse::<studio_runtime_providers::ProviderKind>()
                .map_err(|_| {
                    TransferError::Refused("Choose a valid transfer scope and destination provider")
                })?;
            let target_provider = target_provider.key().to_owned();
            drop_orphan_summary_in(tx, &lead)?;
            let actor = crate::event_write::checked_agent(tx, &lead)
                .map_err(|_| TransferError::Refused("Unknown managed agent"))?;
            if actor.is_lead != Some(true) {
                return Err(TransferError::Refused(
                    "Choose the orchestrator to transfer its team",
                ));
            }
            if excludes_account(&actor, &target) {
                return Err(TransferError::Refused(
                    "This account is disabled for this chat",
                ));
            }
            let summary = match actor.account_transfer.value() {
                Some(studio_runtime_store::transfer_record::AgentTransferState::Unreadable(_)) => {
                    return Err(TransferError::Refused(
                        "Current transfer data is unreadable",
                    ));
                }
                Some(studio_runtime_store::transfer_record::AgentTransferState::Summary(value)) => {
                    Some(value)
                }
                None => None,
            };
            if let Some(active) = summary.filter(|value| {
                !matches!(
                    value.status,
                    studio_runtime_store::transfer_record::TransferStatus::Completed
                        | studio_runtime_store::transfer_record::TransferStatus::Cancelled
                )
            }) {
                let active_scope = match active.scope.value() {
                    Some(studio_runtime_store::transfer_record::TransferScope::Subagents) => {
                        "subagents"
                    }
                    Some(studio_runtime_store::transfer_record::TransferScope::Team) | None => {
                        "team"
                    }
                };
                if active.target_account_key.as_str() != target.as_str() || active_scope != scope {
                    return Err(TransferError::Refused(
                        "Finish or cancel the current transfer first",
                    ));
                }
                let active_id = active.id.as_str();
                let snapshot = load(tx, active_id)?;
                let decision = snapshot.add_team_request(
                    id.clone(),
                    lead.clone(),
                    target
                        .clone()
                        .try_into()
                        .map_err(|_| TransferError::Refused("Invalid transfer target"))?,
                )?;
                return persist_request(tx, decision, now, Some(&id));
            }

            // Only the chat itself moves. Its subagents keep their accounts.
            let mut agents: std::collections::BTreeMap<_, _> =
                Repository::team_agents(tx, &AgentId::from(lead.as_str()))?
                    .into_iter()
                    .filter(|agent| agent.id.0 == lead)
                    .map(|agent| (agent.id.0.clone(), agent))
                    .collect();
            if !agents.contains_key(&lead) {
                return Err(TransferError::Refused("Unknown managed agent"));
            }
            let operation_id = id;
            let target_key = target
                .clone()
                .try_into()
                .map_err(|_| TransferError::Refused("Invalid transfer target"))?;
            let transfer_id = operation_id
                .clone()
                .try_into()
                .map_err(|_| TransferError::Refused("Invalid transfer identity"))?;
            let provider = target_provider
                .parse()
                .map_err(|_| TransferError::Refused("Invalid transfer provider"))?;
            let mut members = Vec::new();
            for (agent_id, agent) in &mut agents {
                members.push((
                    agent_id.clone(),
                    crate::account_transfer::typed_enroll(
                        agent,
                        &transfer_id,
                        &target_key,
                        provider,
                        agent_id == &lead,
                    )?,
                ));
            }
            let operation = TransferRecord::new_team(
                operation_id.clone(),
                lead.clone(),
                target_key,
                provider,
                serde_json::Number::from_f64(now)
                    .ok_or(TransferError::Refused("Invalid transfer time"))?,
                studio_runtime_store::transfer_record::OrderedMembers(members),
            );
            let raw = serde_json::to_string(&operation)
                .map_err(|_| TransferError::Refused("Invalid transfer"))?;
            let snapshot = Snapshot::read_typed(
                operation.clone(),
                raw.len(),
                agents.iter().map(|(id, value)| {
                    Ok((
                        id.clone(),
                        serde_json::to_string(value)
                            .map_err(|_| TransferError::Refused("Invalid transfer agent"))?,
                    ))
                }),
            )?;
            let mut decision = snapshot.decide(Command::Restart)?;
            decision.operation = operation;
            decision.replace_agent_records(agents)?;
            decision.changed = true;
            persist_request(tx, decision, now, Some(&operation_id))
        })
        .await
}

pub async fn reserve(
    store: &RuntimeStore,
    ticket: &Ticket,
    method: &str,
    params: Value,
    scope: Value,
    now: f64,
) -> Result<(), TransferError> {
    let ticket = ticket.clone();
    let method = method.to_owned();
    store
        .transaction_with_error(move |tx| {
            check_durable_blockers(tx, &ticket.agent)?;
            let snapshot = load(tx, &ticket.operation)?;
            let mut decision = typed_intent(
                &snapshot,
                &ticket.agent,
                &ticket.request,
                scope,
                &method,
                params,
            )?;
            admit_mutation(&snapshot, &mut decision, &ticket.agent)?;
            persist(tx, decision, now)?;
            Ok(())
        })
        .await
}
pub async fn receive(
    store: &RuntimeStore,
    ticket: &Ticket,
    result: Option<Value>,
    now: f64,
) -> Result<(), TransferError> {
    let ticket = ticket.clone();
    store
        .transaction_with_error(move |tx| {
            let snapshot = load(tx, &ticket.operation)?;
            let result = result
                .map(|value| serde_json::value::to_raw_value(&value))
                .transpose()
                .map_err(|_| TransferError::Refused("Invalid provider payload"))?;
            persist(tx, snapshot.typed_receive(&ticket, result)?, now)?;
            Ok(())
        })
        .await
}

fn persist(tx: &Transaction<'_>, decision: Decision, now: f64) -> Result<Value, TransferError> {
    persist_request(tx, decision, now, None)
}

fn persist_request(
    tx: &Transaction<'_>,
    mut decision: Decision,
    now: f64,
    request_id: Option<&str>,
) -> Result<Value, TransferError> {
    if !decision.changed {
        return serde_json::to_value(decision.typed_public_view())
            .map_err(|_| TransferError::Refused("Invalid transfer response"));
    }
    let operation = &decision.operation;
    let agents = decision.agent_records()?;
    let old =
        agents
            .get(&operation.lead_id)
            .and_then(|agent| match agent.account_transfer.value() {
                Some(studio_runtime_store::transfer_record::AgentTransferState::Summary(
                    summary,
                )) => Some(summary.id.as_str()),
                _ => None,
            });
    let previous_created = old
        .map(|id| Repository::get_typed_with_raw(tx, id))
        .transpose()?
        .flatten()
        .and_then(|(op, _)| op.created.as_f64());
    decision.stamp(now, previous_created)?;
    if let Some(request_id) = request_id {
        let saved = decision.typed_public_view();
        decision.operation.save_response(request_id, saved)?;
    }
    let raw = decision.encoded()?;
    let operation = &decision.operation;
    Repository::put(tx, &operation.id, &raw)?;
    let mut agents = decision.agent_records()?;
    for (id, agent) in &mut agents {
        let prior = AgentRepository::get(tx, &AgentId::from(id.as_str()))?;
        if let Some(prior) = &prior
            && prior.account_key != agent.account_key
            && let Some(resume) = prior.extra.get("usageResume")
            && matches!(
                resume.get("status").and_then(Value::as_str),
                Some("scheduled" | "started")
            )
        {
            let resume_id =
                resume
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or(TransferError::Refused(
                        "Invalid account transfer resume identity",
                    ))?;
            let cancelled = studio_runtime_store::cancel_usage_resume(
                tx,
                id,
                resume_id,
                "The chat moved to another account.",
                now,
            )
            .map_err(|error| match error {
                studio_runtime_store::UsageRepositoryError::Store(error) => {
                    TransferError::Store(error)
                }
                studio_runtime_store::UsageRepositoryError::MissingFamily
                | studio_runtime_store::UsageRepositoryError::EventDriverRequired
                | studio_runtime_store::UsageRepositoryError::UnknownAgent
                | studio_runtime_store::UsageRepositoryError::UnknownContinuation
                | studio_runtime_store::UsageRepositoryError::InvalidStoredObject { .. }
                | studio_runtime_store::UsageRepositoryError::Sqlite(_) => TransferError::Stored {
                    object: "usage resume",
                    pointer: "/".into(),
                },
            })?;
            agent.extra.insert("usageResume".into(), cancelled);
        }
        if prior.as_ref() == Some(agent) {
            continue;
        }
        AgentRepository::put(tx, agent)?;
        crate::agents_create::publish_agent(tx, &agent.value())?;
    }
    for followup in &decision.followups {
        let agent = agents
            .get(&followup.agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        crate::event_write::enqueue_event(
            tx,
            agent,
            followup.id.clone(),
            "followup",
            followup.text.into(),
            now,
        )
        .map_err(|_| TransferError::Refused("Cannot enqueue transfer continuation"))?;
    }
    serde_json::to_value(decision.typed_public_view())
        .map_err(|_| TransferError::Refused("Invalid transfer response"))
}

/// Asynchronous effect port, implemented by the account worker's bounded native
/// request adapter. A failed wait is unknown, never evidence of non-execution.
pub type TransferFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub trait TransferTransport: Send + Sync {
    fn send<'a>(
        &'a self,
        ticket: &'a Ticket,
        scope: &'a crate::account_session_runtime::ConnectionScope,
        method: &'a str,
        params: &'a Value,
    ) -> TransferFuture<'a, Result<TransferDispatch, TransferError>>;
}
#[derive(Debug, PartialEq, Eq)]
pub enum DispatchOutcome {
    ReceiptSaved,
    Unknown,
}
/// The returned ticket reservation is committed before `send` is invoked.
/// Callers must not retry after any error; inspect the durable operation first.
pub async fn send_once(
    store: &RuntimeStore,
    transport: &dyn TransferTransport,
    ticket: &Ticket,
    scope: crate::account_session_runtime::ConnectionScope,
    method: &str,
    params: Value,
    now: f64,
) -> Result<DispatchOutcome, TransferError> {
    let scope_value = serde_json::to_value(&scope)
        .map_err(|_| TransferError::Refused("Invalid provider scope"))?;
    reserve(store, ticket, method, params.clone(), scope_value, now).await?;
    let dispatch = transport.send(ticket, &scope, method, &params).await?;
    let known = dispatch.has_thread();
    provider_receipt(
        store,
        ProviderReceipt {
            operation: &ticket.operation,
            agent: &ticket.agent,
            provider_operation: &ticket.request,
            receipt: dispatch,
        },
        now,
    )
    .await?;
    Ok(if known {
        DispatchOutcome::ReceiptSaved
    } else {
        DispatchOutcome::Unknown
    })
}

/// Called after the account worker has checked its live preparation/catalog
/// reservations. Durable local blockers are checked again in this transaction.
pub async fn begin_read(
    store: &RuntimeStore,
    operation: &str,
    agent: &str,
    now: f64,
) -> Result<(), TransferError> {
    let operation = operation.to_owned();
    let agent = agent.to_owned();
    store
        .transaction_with_error(move |tx| {
            check_durable_blockers(tx, &agent)?;
            persist(tx, load(tx, &operation)?.begin_read(&agent)?, now)?;
            Ok(())
        })
        .await
}

/// Persist a stable provider operation and its observed connection scope after
/// history/settings preparation and before calling the retained session.
pub struct ProviderIntent<'a> {
    pub operation: &'a str,
    pub agent: &'a str,
    pub provider_operation: &'a str,
    pub scope: Value,
    pub method: &'a str,
    pub params: Value,
}

pub async fn provider_intent(
    store: &RuntimeStore,
    intent: ProviderIntent<'_>,
    now: f64,
) -> Result<(), TransferError> {
    let operation = intent.operation.to_owned();
    let agent = intent.agent.to_owned();
    let provider_operation = intent.provider_operation.to_owned();
    let method = intent.method.to_owned();
    store
        .transaction_with_error(move |tx| {
            let snapshot = load(tx, &operation)?;
            let mut decision = typed_intent(
                &snapshot,
                &agent,
                &provider_operation,
                intent.scope,
                &method,
                intent.params,
            )?;
            admit_mutation(&snapshot, &mut decision, &agent)?;
            persist(tx, decision, now)?;
            Ok(())
        })
        .await
}

fn typed_intent(
    snapshot: &Snapshot,
    agent: &str,
    operation: &str,
    scope: Value,
    method: &str,
    params: Value,
) -> Result<Decision, TransferError> {
    snapshot.typed_provider_intent(
        agent,
        operation,
        serde_json::from_value(scope)
            .map_err(|_| TransferError::Refused("Invalid provider scope"))?,
        serde_json::from_value(Value::String(method.into()))
            .map_err(|_| TransferError::Refused("Invalid provider method"))?,
        &serde_json::value::to_raw_value(&params)
            .map_err(|_| TransferError::Refused("Invalid provider payload"))?,
    )
}

pub(crate) struct ProviderReadIntent<'a> {
    pub transfer: &'a str,
    pub agent: &'a str,
    pub operation: &'a str,
    pub account: &'a str,
    pub scope: Value,
    pub method: &'a str,
    pub params: Value,
}

/// The root is created only with a new transfer obligation. Imported obligations
/// carry their final provider token from the migration.
pub(crate) use studio_runtime_store::transfer_record::ProviderAdmission;

fn admit_mutation(
    snapshot: &Snapshot,
    decision: &mut Decision,
    agent: &str,
) -> Result<(), TransferError> {
    let member = snapshot
        .record()
        .members
        .get(agent)
        .ok_or(TransferError::Refused("Unknown transfer member"))?;
    if member.provider_intent.is_missing() {
        use studio_runtime_schema::queue_command::Field;
        let member = decision
            .operation
            .members
            .get_mut(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        member.provider_admission = Field::Value(ProviderAdmission {
            root_request_id: studio_runtime_schema::persistence::RequestIdentity::issue_root(),
            admitted_request: Field::Null,
        });
        decision.changed = true;
    } else if member.provider_admission.is_missing() {
        return Err(TransferError::Refused(
            "The transfer provider admission is missing",
        ));
    }
    Ok(())
}

pub(crate) async fn provider_admission(
    store: &RuntimeStore,
    transfer: &str,
    agent: &str,
    operation: &str,
) -> Result<ProviderAdmission, TransferError> {
    let transfer = transfer.to_owned();
    let agent = agent.to_owned();
    let operation = operation.to_owned();
    store
        .read_async("account-transfer/provider-admission", move |tx| {
            let snapshot = load(tx, &transfer)?;
            let member = snapshot
                .record()
                .members
                .get(&agent)
                .ok_or(TransferError::Refused(
                    "The transfer provider admission is missing",
                ))?;
            let admission = if member
                .provider_intent
                .value()
                .is_some_and(|intent| intent.operation == operation)
            {
                member.provider_admission.value()
            } else {
                member
                    .provider_read_admissions
                    .value()
                    .and_then(|admissions| admissions.get(&operation))
            };
            admission.cloned().ok_or(TransferError::Refused(
                "The transfer provider admission is missing",
            ))
        })
        .await
}

pub(crate) async fn provider_read_intent(
    store: &RuntimeStore,
    intent: ProviderReadIntent<'_>,
    now: f64,
) -> Result<Option<studio_runtime_store::transfer_record::ProviderReadReceipt>, TransferError> {
    let transfer = intent.transfer.to_owned();
    let agent = intent.agent.to_owned();
    let operation = intent.operation.to_owned();
    let account = intent.account.to_owned();
    let method = intent.method.to_owned();
    let root = store.root().to_path_buf();
    store
        .transaction_with_error(move |tx| {
            let snapshot = load(tx, &transfer)?;
            use studio_runtime_store::transfer_record::{
                ProviderIntent as StoredIntent, ReceiptCertainty,
            };
            let next = StoredIntent {
                operation: operation.clone(),
                account: Field::Value(
                    account
                        .clone()
                        .try_into()
                        .map_err(|_| TransferError::Refused("Invalid provider account"))?,
                ),
                scope: serde_json::from_value(intent.scope.clone())
                    .map_err(|_| TransferError::Refused("Invalid provider scope"))?,
                method: serde_json::from_value(Value::String(method.clone()))
                    .map_err(|_| TransferError::Refused("Invalid provider method"))?,
                params: serde_json::value::to_raw_value(&intent.params)
                    .map_err(|_| TransferError::Refused("Invalid provider parameters"))?,
            };
            if let Some(reply) = snapshot.written_read_replay_typed(&agent, &next)? {
                return Ok(Some(inline_reply(&root, reply)?));
            }
            let member = snapshot
                .record()
                .members
                .get(&agent)
                .ok_or(TransferError::Refused("Unknown transfer member"))?;
            let prior = member
                .provider_read_receipts
                .value()
                .and_then(|receipts| receipts.get(&operation))
                .filter(|receipt| receipt.receipt.certainty == ReceiptCertainty::Written)
                .cloned()
                .map(|saved| inline_reply(&root, saved))
                .transpose()?;
            let mut decision = snapshot.typed_provider_read_intent(&agent, &next)?;
            if method == "thread/unsubscribe" {
                if member
                    .provider_read_intents
                    .value()
                    .and_then(|intents| intents.get(&operation))
                    .is_none()
                {
                    decision.admit_provider_read(
                        &agent,
                        &operation,
                        ProviderAdmission {
                            root_request_id:
                                studio_runtime_schema::persistence::RequestIdentity::issue_root(),
                            admitted_request: Field::Null,
                        },
                    )?;
                } else if member
                    .provider_read_admissions
                    .value()
                    .and_then(|admissions| admissions.get(&operation))
                    .is_none()
                {
                    return Err(TransferError::Refused(
                        "The transfer provider admission is missing",
                    ));
                }
            }
            persist(tx, decision, now)?;
            Ok(prior)
        })
        .await
}

/// Put back a reply body that the record keeps as a payload blob.
fn inline_reply(
    root: &std::path::Path,
    mut saved: studio_runtime_store::transfer_record::ProviderReadReceipt,
) -> Result<studio_runtime_store::transfer_record::ProviderReadReceipt, TransferError> {
    let Some(result) = (match &mut saved.result {
        Field::Value(result) => Some(result),
        Field::Missing | Field::Null => None,
    }) else {
        return Ok(saved);
    };
    let (kind, reference) = match result {
        studio_runtime_store::transfer_record::ProviderResult::Inline { .. } => return Ok(saved),
        studio_runtime_store::transfer_record::ProviderResult::Blob { kind, value_blob } => {
            (kind.clone(), value_blob)
        }
    };
    let lost = |_| TransferError::Refused("The saved provider reply is unavailable");
    let reference = serde_json::to_value(reference).map_err(lost)?;
    let bytes = crate::payload_blobs::load_blob(root, &reference)
        .map_err(|_| TransferError::Refused("The saved provider reply is unavailable"))?;
    let value = serde_json::from_slice(&bytes).map_err(lost)?;
    *result = studio_runtime_store::transfer_record::ProviderResult::Inline {
        kind,
        value: Some(value),
    };
    Ok(saved)
}

pub(crate) struct ProviderReadReceipt<'a> {
    pub transfer: &'a str,
    pub agent: &'a str,
    pub operation: &'a str,
    pub receipt: crate::account_session_runtime::SessionReceipt,
}

pub(crate) async fn provider_read_receipt(
    store: &RuntimeStore,
    receipt: ProviderReadReceipt<'_>,
    now: f64,
) -> Result<crate::account_session_runtime::SessionReceipt, TransferError> {
    let transfer = receipt.transfer.to_owned();
    let agent = receipt.agent.to_owned();
    let operation = receipt.operation.to_owned();
    // The blob is content addressed: a failed transaction leaves a harmless file.
    if let Some((sha256, bytes)) = crate::account_transfer::spilled_reply(&receipt.receipt) {
        crate::payload_blobs::write_blobs(
            store.root(),
            &[crate::payload_blobs::BlobWrite {
                sha256,
                bytes,
                preview: Value::Null,
            }],
        )
        .map_err(|_| TransferError::Refused("The provider reply could not be saved"))?;
    }
    store
        .transaction_with_error(move |tx| {
            let snapshot = load(tx, &transfer)?;
            persist(
                tx,
                snapshot.provider_read_receipt(&agent, &operation, &receipt.receipt)?,
                now,
            )?;
            Ok(receipt.receipt)
        })
        .await
}

/// Persist the provider's exact ticket and certainty before adopting any returned
/// thread. Unknown stays held; only a proven NotSent receipt can be retried.
pub struct ProviderReceipt<'a> {
    pub operation: &'a str,
    pub agent: &'a str,
    pub provider_operation: &'a str,
    pub receipt: TransferDispatch,
}

pub async fn provider_receipt(
    store: &RuntimeStore,
    receipt: ProviderReceipt<'_>,
    now: f64,
) -> Result<(), TransferError> {
    let operation = receipt.operation.to_owned();
    let agent = receipt.agent.to_owned();
    let provider_operation = receipt.provider_operation.to_owned();
    store
        .transaction_with_error(move |tx| {
            let snapshot = load(tx, &operation)?;
            persist(
                tx,
                snapshot.provider_receipt(&agent, &provider_operation, &receipt.receipt)?,
                now,
            )?;
            Ok(())
        })
        .await
}

pub async fn prepared_member(
    store: &RuntimeStore,
    operation: &str,
    agent: &str,
    target_settings: Value,
    portable_history: Option<Value>,
    now: f64,
) -> Result<(), TransferError> {
    let operation = operation.to_owned();
    let agent = agent.to_owned();
    store
        .transaction_with_error(move |tx| {
            let snapshot = load(tx, &operation)?;
            persist(
                tx,
                snapshot.prepared_member(&agent, target_settings, portable_history)?,
                now,
            )?;
            Ok(())
        })
        .await
}

pub async fn adopt_receipt(
    store: &RuntimeStore,
    operation: &str,
    agent: &str,
    now: f64,
) -> Result<Value, TransferError> {
    let operation = operation.to_owned();
    let agent = agent.to_owned();
    store
        .transaction_with_error(move |tx| {
            let snapshot = load(tx, &operation)?;
            persist(tx, snapshot.adopt_receipt(&agent)?, now)
        })
        .await
}

fn check_durable_blockers(tx: &Transaction<'_>, agent: &str) -> Result<(), TransferError> {
    for table in ["runtime_tasks", "runtime_requests"] {
        let sql = format!(
            "SELECT EXISTS(SELECT 1 FROM {table} WHERE json_extract(record,'$.agent')=?1 AND json_extract(record,'$.status') IN ('running','starting','approval','pending','unknown'))"
        );
        let busy: bool = tx
            .query_row(&sql, [agent], |r| r.get(0))
            .map_err(studio_runtime_store::StoreError::from)?;
        if busy {
            return Err(TransferError::Refused(
                "Waiting for background work or a tool response",
            ));
        }
    }
    let mut monitors=tx.prepare_cached("SELECT id FROM runtime_monitors WHERE json_extract(record,'$.agent')=?1 AND json_extract(record,'$.status') IN ('running','starting','approval','pending','unknown')").map_err(studio_runtime_store::StoreError::from)?;
    let monitors = monitors
        .query_map([agent], |row| row.get::<_, String>(0))
        .map_err(studio_runtime_store::StoreError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(studio_runtime_store::StoreError::from)?;
    for id in monitors {
        if !crate::account_worker::retained::monitor_survives_account_transfer(tx, &id, agent)
            .map_err(|error| match error {
                crate::account_worker::WorkerError::Store(error) => TransferError::Store(error),
                crate::account_worker::WorkerError::Expired => TransferError::Expired,
                crate::account_worker::WorkerError::Recovery(_)
                | crate::account_worker::WorkerError::Scheduler(_)
                | crate::account_worker::WorkerError::Session(_)
                | crate::account_worker::WorkerError::Notification(_)
                | crate::account_worker::WorkerError::PreparationRefused(_)
                | crate::account_worker::WorkerError::StaleVmActor
                | crate::account_worker::WorkerError::PreparationPending(_)
                | crate::account_worker::WorkerError::PreparationAwaitsAgent(_)
                | crate::account_worker::WorkerError::CleanupUnconfirmed(_)
                | crate::account_worker::WorkerError::AlreadyRunning
                | crate::account_worker::WorkerError::Closed
                | crate::account_worker::WorkerError::ShutdownTimeout
                | crate::account_worker::WorkerError::TaskFailed
                | crate::account_worker::WorkerError::Capacity
                | crate::account_worker::WorkerError::CallbackUnknown => {
                    TransferError::Refused("Source monitor process ownership could not be verified")
                }
            })?
        {
            return Err(TransferError::Refused(
                "Waiting for verified source monitor ownership. The original process was not stopped.",
            ));
        }
    }
    let uncertain: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM runtime_events WHERE agent=?1 AND status IN ('reserved','dispatching','uncertain'))",[agent],|r| r.get(0)).map_err(studio_runtime_store::StoreError::from)?;
    if uncertain {
        return Err(TransferError::Refused(
            "Waiting for confirmed message delivery",
        ));
    }
    Ok(())
}

/// Captured after the durable reading intent. History and destination settings
/// are prepared outside the writer transaction from these checked records.
pub struct TransferPreparation {
    operation: TransferRecord,
    agent: Value,
}
impl TransferPreparation {
    pub fn operation(&self) -> &TransferRecord {
        &self.operation
    }
    pub fn agent(&self) -> &Value {
        &self.agent
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferMethod {
    Start,
    Fork,
}
impl TransferMethod {
    pub(crate) fn wire(self) -> &'static str {
        match self {
            Self::Start => "thread/start",
            Self::Fork => "thread/fork",
        }
    }
}
pub struct PreparedTransfer {
    pub ticket: Ticket,
    pub scope: crate::account_session_runtime::ConnectionScope,
    pub method: TransferMethod,
    pub params: Value,
}

/// P07 supplies history/settings preparation; P01 supplies the retained transport.
/// `available` must refuse before any write if either dependency is absent.
/// Preparation must be bounded and safe to repeat for the same operation/member;
/// it must not send a native mutation. The transport must use the supplied ticket.
pub trait TransferExecutor: TransferTransport {
    fn available(&self) -> Result<(), TransferError>;
    fn prepare<'a>(
        &'a self,
        input: &'a TransferPreparation,
    ) -> TransferFuture<'a, Result<PreparedTransfer, TransferError>>;
}

/// Execute one existing member through its durable receipt. Adoption remains a
/// separate transaction owned by the history/settings/resume-cancellation adapter.
/// Call `recover` once at startup, before running members or admitting turns.
pub async fn execute_member(
    store: &RuntimeStore,
    executor: &dyn TransferExecutor,
    operation: &str,
    agent: &str,
    now: f64,
) -> Result<DispatchOutcome, TransferError> {
    executor.available()?;
    let operation_id = operation.to_owned();
    let agent_id = agent.to_owned();
    let transaction_agent_id = agent_id.clone();
    let input = store
        .transaction_with_error(move |tx| {
            if let Err(error) = check_durable_blockers(tx, &transaction_agent_id) {
                if let TransferError::Refused(reason) = &error {
                    let snapshot = load(tx, &operation_id)?;
                    persist(
                        tx,
                        snapshot.waiting_for_blocker(&transaction_agent_id, reason)?,
                        now,
                    )?;
                }
                return Ok(Err(error));
            }
            let snapshot = load(tx, &operation_id)?;
            let decision = snapshot.begin_read(&transaction_agent_id)?;
            let input = TransferPreparation {
                operation: decision.operation.clone(),
                agent: snapshot.agent_record(&transaction_agent_id)?.value(),
            };
            persist(tx, decision, now)?;
            Ok::<_, TransferError>(Ok(input))
        })
        .await??;
    let prepared = executor.prepare(&input).await.and_then(|prepared| {
        if prepared.ticket.operation != operation || prepared.ticket.agent != agent {
            return Err(TransferError::Refused(
                "The prepared transfer belongs to another member",
            ));
        }
        Ok(prepared)
    });
    let result = match prepared {
        Ok(prepared) => {
            send_once(
                store,
                executor,
                &prepared.ticket,
                prepared.scope,
                prepared.method.wire(),
                prepared.params,
                now,
            )
            .await
        }
        Err(error) => Err(error),
    };
    if result.is_err() {
        let operation_id = operation.to_owned();
        let failed_agent = agent_id.clone();
        store
            .transaction_with_error(move |tx| {
                let snapshot = load(tx, &operation_id)?;
                persist(
                    tx,
                    snapshot.typed_preparation_failed(
                        &failed_agent,
                        input
                            .operation
                            .members
                            .get(&failed_agent)
                            .map_or(&Field::Missing, |member| &member.source),
                    )?,
                    now,
                )?;
                Ok::<_, TransferError>(())
            })
            .await?;
    }
    result
}

#[cfg(test)]
mod request_tests {
    use super::request as request_async;
    use super::*;
    use serde_json::json;
    use std::future::Future;
    use studio_runtime_store::{AgentId, AgentRecord, AgentRepository};

    fn run_async<F: Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime builds")
            .block_on(future)
    }

    fn request(
        store: &RuntimeStore,
        id: &str,
        lead: &str,
        target: &str,
        scope: &str,
        provider: &str,
        now: f64,
    ) -> Result<Value, TransferError> {
        run_async(request_async(store, id, lead, target, scope, provider, now))
    }

    fn store_with_agents() -> (tempfile::TempDir, RuntimeStore) {
        let root = tempfile::tempdir().expect("test fixture operation succeeds");
        let store = run_async(RuntimeStore::open(Some(root.path())))
            .expect("test fixture operation succeeds");
        let cwd = store.root().to_path_buf();
        run_async(store.transaction_with_error(move |tx| {
            for value in [
                json!({"compactions":0,"id":"lead","rootId":"lead","epoch":1,"status":"idle","isLead":true,
                    "accountKey":"source","provider":"codex","model":"gpt-6-astra","cwd":cwd}),
                json!({"compactions":0,"id":"child","rootId":"lead","epoch":2,"status":"idle","isLead":false,
                    "accountKey":"source","provider":"codex","model":"gpt-6-astra","cwd":cwd}),
            ] {
                let agent =
                    AgentRecord::from_value(value).expect("test fixture operation succeeds");
                AgentRepository::put(tx, &agent).expect("test fixture operation succeeds");
            }
            Ok::<_, studio_runtime_store::StoreError>(())
        }))
        .expect("test fixture operation succeeds");
        (root, store)
    }

    fn add_agents(store: &RuntimeStore, root: &str, count: usize) {
        let root = root.to_owned();
        let cwd = store.root().to_path_buf();
        run_async(store.transaction(move |tx| {
            let template = AgentRecord::from_value(json!({"compactions":0,
                "id":"template", "rootId":root, "epoch":1, "status":"idle",
                "isLead":false, "accountKey":"source", "provider":"codex",
                "model":"gpt-6-astra", "cwd":cwd
            }))?.value().to_string();
            tx.execute(
                "WITH RECURSIVE members(n) AS (SELECT 1 WHERE ?1>0 UNION ALL SELECT n+1 FROM members WHERE n<?1)
                 INSERT INTO runtime_agents(id,record)
                 SELECT 'fixture-'||n,json_set(?2,'$.id','fixture-'||n) FROM members",
                rusqlite::params![i64::try_from(count).expect("fixture count fits SQLite"), template],
            )?;
            Ok(())
        })).expect("seed scoped fixture agents");
    }

    #[test]
    fn new_request_ignores_more_than_4096_unrelated_agents() {
        let (_root, store) = store_with_agents();
        add_agents(&store, "another-team", 4097);
        let unrelated = stored(&store, "fixture-4097");
        let id = "5a300f72-4ea6-4d30-8a0e-9b2c13c42280";
        let result = request(&store, id, "lead", "target", "team", "codex", 10.0)
            .expect("unrelated database size cannot prevent a team transfer");
        assert_eq!(result["members"].as_object().expect("members").len(), 1);
        assert_eq!(stored(&store, "fixture-4097"), unrelated);
        assert_eq!(
            request(&store, id, "lead", "target", "team", "codex", 11.0).expect("exact replay"),
            result
        );
        assert!(request(&store, id, "lead", "changed", "team", "codex", 12.0).is_err());
    }

    #[test]
    fn chat_request_enrolls_only_the_lead_and_leaves_subagents_alone() {
        let (_root, store) = store_with_agents();
        let before = stored(&store, "lead");
        let result = request(
            &store,
            "5a300f72-4ea6-4d30-8a0e-9b2c13c42290",
            "lead",
            "target",
            "team",
            "codex",
            10.0,
        )
        .expect("chat request");
        let members = result["members"].as_object().expect("members");
        assert_eq!(members.keys().collect::<Vec<_>>(), ["lead"]);
        assert_eq!(stored(&store, "lead")["accountKey"], "target");
        assert_eq!(stored(&store, "child")["accountKey"], "source");
        assert_eq!(
            stored(&store, "lead")["workerDefaults"],
            before["workerDefaults"]
        );
        run_async(store.transaction(|tx| {
            let late = AgentRecord::from_value(
                json!({"compactions":0,"id":"late","rootId":"lead","epoch":3,
                "status":"idle","provider":"codex","accountKey":"source"}),
            )?;
            AgentRepository::put(tx, &late)
        }))
        .expect("late subagent");
        let again = request(
            &store,
            "5a300f72-4ea6-4d30-8a0e-9b2c13c42291",
            "lead",
            "target",
            "team",
            "codex",
            11.0,
        )
        .expect("active chat request");
        assert_eq!(again["members"].as_object().expect("members").len(), 1);
        assert_eq!(stored(&store, "late")["accountKey"], "source");
    }

    #[test]
    fn inline_reply_restores_a_spilled_body() {
        let root = tempfile::tempdir().expect("directory");
        let body = json!({"data":["page"]});
        let bytes = serde_json::to_vec(&body).expect("bytes");
        let sha256 = studio_runtime_store::sha256_hex(&bytes);
        crate::payload_blobs::write_blobs(
            root.path(),
            &[crate::payload_blobs::BlobWrite {
                sha256: sha256.clone(),
                bytes: bytes.clone(),
                preview: Value::Null,
            }],
        )
        .expect("blob");
        let receipt = json!({"operation":"read", "scope":{"account":"source","connection":"c","generation":1}, "certainty":"written"});
        let saved = json!({"certainty":"written", "receipt":receipt, "result":{"kind":"reply","valueBlob":{"sha256":sha256,"size":bytes.len()}}});
        let saved = serde_json::from_value(saved).expect("receipt");
        let restored = inline_reply(root.path(), saved).expect("restored");
        assert_eq!(
            serde_json::to_value(restored).unwrap()["result"],
            json!({"kind":"reply","value":body})
        );
        let lost = json!({"certainty":"written", "receipt":receipt, "result":{"kind":"reply","valueBlob":{"sha256":"0".repeat(64),"size":1}}});
        assert!(inline_reply(root.path(), serde_json::from_value(lost).unwrap()).is_err());
    }

    #[test]
    fn account_transfer_startup_holds_unsupported_operation() {
        let (root, store) = store_with_agents();
        drop(store);
        let raw = r#"{"id":"invalid","leadId":"lead","targetAccountKey":"target","targetProvider":"codex","status":"pending","created":1,"members":{},"futureField":true}"#;
        let db = rusqlite::Connection::open(root.path().join("canvas.sqlite3")).unwrap();
        db.execute(
            "INSERT INTO runtime_account_transfers(id,record) VALUES('invalid',?1)",
            [raw],
        )
        .unwrap();
        db.execute(
            "DELETE FROM sync_entity_meta WHERE key='migration_catalog'",
            [],
        )
        .unwrap();
        drop(db);
        let store = run_async(RuntimeStore::open(Some(root.path()))).expect("store starts");
        assert_eq!(
            run_async(recover(&store, 10.0)).expect("recovery skips held operation"),
            0
        );
        let db = store.test_reader().unwrap();
        let (kind, original): (String, String) = db.query_row("SELECT json_extract(record,'$.kind'),json_extract(record,'$.original') FROM runtime_account_transfers WHERE id='invalid'", [], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
        assert_eq!(kind, "unreadable");
        assert_eq!(original, raw);
        let marker: String = db.query_row("SELECT json_extract(record,'$.accountTransfer.kind') FROM runtime_agents WHERE id='lead'", [], |row| row.get(0)).unwrap();
        assert_eq!(marker, "unreadable");
        drop(db);
        drop(store);
        let reopened =
            run_async(RuntimeStore::open(Some(root.path()))).expect("second store reopen");
        assert_eq!(
            run_async(recover(&reopened, 11.0)).expect("held operation remains skipped"),
            0
        );
    }

    #[test]
    fn recovery_does_not_hide_unknown_or_missing_operation_status() {
        for status in [json!("future-status"), Value::Null] {
            let (_root, store) = store_with_agents();
            run_async(store.transaction(move |tx| {
                let record = json!({"id":"invalid", "leadId":"lead", "targetAccountKey":"target",
                    "status":status, "created":1.0, "members":{}})
                .to_string();
                tx.execute(
                    "INSERT INTO runtime_account_transfers(id,record) VALUES('invalid',?1)",
                    [record],
                )?;
                Ok(())
            }))
            .expect("invalid operation fixture");
            let before = stored(&store, "lead");
            assert!(
                run_async(recover(&store, 10.0)).is_err(),
                "invalid status must reach the validator"
            );
            assert_eq!(stored(&store, "lead"), before);
        }
    }

    #[test]
    fn unrelated_transfer_history_does_not_block_requests_or_pending_recovery() {
        let (_root, store) = store_with_agents();
        run_async(store.transaction(|tx| {
            let template = json!({"id":"history", "leadId":"lead", "targetAccountKey":"target",
                "status":"completed", "scope":"team", "created":1.0, "members":{}, "requests":{}}).to_string();
            tx.execute(
                "WITH RECURSIVE history(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM history WHERE n<4097)
                 INSERT INTO runtime_account_transfers(id,record)
                 SELECT 'history-'||n,json_set(?1,'$.id','history-'||n) FROM history",
                [template],
            )?;
            Ok(())
        })).expect("seed completed transfer history");
        let id = "5a300f72-4ea6-4d30-8a0e-9b2c13c42285";
        let result = request(&store, id, "lead", "target", "team", "codex", 10.0)
            .expect("history size cannot prevent a new transfer");
        assert_eq!(
            request(&store, id, "lead", "target", "team", "codex", 11.0).expect("exact replay"),
            result
        );
        assert_eq!(
            run_async(recover(&store, 12.0)).expect("pending recovery ignores completed history"),
            0
        );
    }

    #[test]
    fn saved_alias_outcomes_stay_bounded_with_many_prior_requests() {
        let (_root, store) = store_with_agents();
        let original = "5a300f72-4ea6-4d30-8a0e-9b2c13c42270";
        let alias = "5a300f72-4ea6-4d30-8a0e-9b2c13c42271";
        request(&store, original, "lead", "target", "team", "codex", 10.0).unwrap();
        run_async(store.transaction(move |tx| {
            let mut op = Repository::get(tx, original)?.unwrap();
            for i in 0..256 {
                op["requests"][format!("prior-{i:04}")] =
                    json!({"leadId":"lead","targetAccountKey":"target","scope":"team"});
            }
            Repository::put(tx, original, &op.to_string())
        }))
        .unwrap();
        let response = request(&store, alias, "lead", "target", "team", "codex", 11.0).unwrap();
        assert!(response.get("requests").is_none());
        let saved = run_async(store.read_async("alias-size-test", move |tx| {
            Ok::<_, studio_runtime_store::StoreError>(
                Repository::get(tx, original)?.unwrap()["requests"][alias]["savedResponse"]
                    .to_string(),
            )
        }))
        .unwrap();
        assert!(!saved.contains("\"requests\""));
        assert!((saved.len() + 200) * 4096 < studio_runtime_store::MAX_TRANSFER_RECORD_BYTES);
    }

    #[test]
    fn request_identity_is_global_even_between_leads() {
        let (_root, store) = store_with_agents();
        let id = "5a300f72-4ea6-4d30-8a0e-9b2c13c42286";
        let original =
            request(&store, id, "lead", "target", "team", "codex", 10.0).expect("original request");
        run_async(store.transaction(|tx| {
            let other = AgentRecord::from_value(
                json!({"compactions":0,"id":"other", "rootId":"other", "epoch":1,
                "status":"idle", "isLead":true, "provider":"codex", "accountKey":"source"}),
            )?;
            AgentRepository::put(tx, &other)
        }))
        .expect("other lead");
        let before = stored(&store, "other");
        let error = request(&store, id, "other", "target", "team", "codex", 11.0)
            .expect_err("request identity cannot cross leads");
        assert_eq!(error.to_string(), "This transfer id has different content");
        assert_eq!(stored(&store, "other"), before);
        assert_eq!(
            request(&store, id, "lead", "target", "team", "codex", 12.0).expect("original replay"),
            original
        );
    }

    #[test]
    fn exact_replay_stops_reading_members_at_the_snapshot_byte_boundary() {
        let (_root, store) = store_with_agents();
        let id = "5a300f72-4ea6-4d30-8a0e-9b2c13c42293";
        run_async(store.transaction(move |tx| {
            let mut operation = json!({"id":id, "leadId":"lead", "targetAccountKey":"target",
                "status":"pending", "scope":"team", "created":1.0, "members":{}, "requests":{}});
            let member = json!({"phase":"waiting", "sourceAccountKey":"source", "lazy":false});
            operation["members"]["lead"] = member.clone();
            // Each valid row fits the per-record limit; their combined size
            // crosses the snapshot bound before the final oversized sentinel.
            let padding_bytes = studio_runtime_store::MAX_TRANSFER_RECORD_BYTES - 1024 * 1024;
            let template = json!({"compactions":0,"accountKey":"default","id":"padding", "rootId":"lead", "epoch":1, "status":"idle",
                "provider":"codex", "fixturePadding":"x".repeat(padding_bytes)}).to_string();
            let count = Repository::MAX_SNAPSHOT_BYTES / padding_bytes + 1;
            for index in 0..count {
                let member_id = format!("padding-{index:02}");
                tx.execute("INSERT INTO runtime_agents(id,record) VALUES(?1,json_set(?2,'$.id',?1))",
                    rusqlite::params![member_id, template])?;
                operation["members"][&member_id] = member.clone();
            }
            tx.execute("INSERT INTO runtime_agents(id,record) VALUES('z-sentinel',json_set(?1,'$.id','z-sentinel','$.fixturePadding',?2))",
                rusqlite::params![template, "x".repeat(studio_runtime_store::MAX_TRANSFER_RECORD_BYTES)])?;
            operation["members"]["z-sentinel"] = member;
            Repository::put(tx, id, &operation.to_string())
        })).expect("bounded snapshot fixture");
        let before = stored(&store, "lead");
        let error = request(&store, id, "lead", "target", "team", "codex", 10.0)
            .expect_err("exact replay rejects aggregate overflow before reading later members");
        assert!(
            matches!(error, TransferError::Stored { object: "transfer snapshot", ref pointer } if pointer == "/resourceLimit"),
            "snapshot bound must win before the oversized unread sentinel: {error}"
        );
        assert_eq!(stored(&store, "lead"), before);
    }

    #[test]
    fn a_running_retained_monitor_does_not_block_its_owners_account_transfer() {
        let (root, store) = store_with_agents();
        let cwd = root.path().to_owned();
        run_async(store.transaction_with_error(move |tx| {
            use crate::account_worker::retained::{ProviderCallIntent, RequestActor, reserve_provider_call};
            use studio_runtime_schema::persistence::{OutboxStatus, RequestIdentity};
            use studio_runtime_store::request_results::{Admission, RequestResults};
            let root = RequestIdentity::issue_root();
            let owner_id = RequestIdentity::new("monitor-fixture", &[], &root, &[]).expect("root");
            let owner = match RequestResults::admit(tx,&owner_id,"fixture",chrono::Utc::now().timestamp_millis()).map_err(studio_runtime_store::StoreError::from)? {
                Admission::Fresh(owner) => owner,
                Admission::Replay(_) => panic!("fresh fixture root"),
            };
            let method = studio_runtime_providers::Method::CommandExec;
            let admitted = reserve_provider_call(tx,&owner,ProviderCallIntent {
                root_request_id: root.clone(),admitted_request:None,
                scope: crate::account_session_runtime::ConnectionScope {account:"source".into(),connection:"original".into(),generation:4},
                operation:"monitor:watch".into(),actor:Some(RequestActor {agent:"lead".into(),epoch:1}),
                transfer:None,deleted_actor_session:None,method,
                params:json!({"processId":"watch","cwd":cwd,"command":["true"]}),
            })?;
            tx.execute("INSERT INTO runtime_monitors(id,record) VALUES('watch',?1)",
                [crate::monitor_policy::complete_test_monitor(json!({"id":"watch","agent":"lead","epoch":1,"status":"running","operation":{"agent":"lead","epoch":1,"accountKey":"source","connectionId":"original","rootRequestId":root,"providerRequest":admitted}})).to_string()])?;
            let key=studio_runtime_schema::provider_call_contract::transport_key("source","monitor:watch");
            let prepared = studio_runtime_schema::provider_call_contract::TransportRequest {
                id:studio_runtime_schema::provider_input::RequestId::String(key.clone()),generation:4,method,
                operation:key.clone(),thread:None,lightweight:true,deadline:60000,unknown:false,remote_id:None,
            };
            tx.execute("UPDATE runtime_outbox SET status=?2,payload=json_set(payload,'$.payload.request',json(?3),'$.payload.written',json('true')) WHERE id=?1",
                (key,OutboxStatus::Unknown,serde_json::to_string(&prepared).expect("transport request")))?;
            Ok::<_,crate::account_worker::WorkerError>(())
        })).expect("persist the original written command");
        let transfer = "5a300f72-4ea6-4d30-8a0e-9b2c13c4a001";
        request(&store, transfer, "lead", "target", "team", "codex", 1.0)
            .expect("request the account transfer");
        run_async(begin_read(&store, transfer, "lead", 2.0))
            .expect("the model account can move without restarting its original process");
        let monitor = studio_runtime_store::MonitorsRepository::get(
            &store.test_reader().expect("reader"),
            "watch",
        )
        .expect("read monitor")
        .expect("monitor remains");
        assert_eq!(
            monitor.status,
            studio_runtime_schema::studio_errors::MonitorStatus::Running
        );
        assert_eq!(
            monitor
                .operation
                .as_ref()
                .expect("operation")
                .account_key
                .as_str(),
            "source"
        );
    }

    #[test]
    fn an_unproven_monitor_publishes_one_transfer_wait_without_repeated_writes() {
        struct NoNative;
        impl TransferTransport for NoNative {
            fn send<'a>(
                &'a self,
                _: &'a Ticket,
                _: &'a crate::account_session_runtime::ConnectionScope,
                _: &'a str,
                _: &'a Value,
            ) -> TransferFuture<'a, Result<TransferDispatch, TransferError>> {
                Box::pin(async { panic!("an unproven monitor cannot dispatch a transfer") })
            }
        }
        impl TransferExecutor for NoNative {
            fn available(&self) -> Result<(), TransferError> {
                Ok(())
            }
            fn prepare<'a>(
                &'a self,
                _: &'a TransferPreparation,
            ) -> TransferFuture<'a, Result<PreparedTransfer, TransferError>> {
                Box::pin(async { panic!("an unproven monitor cannot prepare a transfer") })
            }
        }
        let (_root, store) = store_with_agents();
        run_async(store.transaction(|tx| {
            tx.execute("INSERT INTO runtime_monitors(id,record) VALUES('watch',?1)",[crate::monitor_policy::complete_test_monitor(json!({"id":"watch","agent":"lead","epoch":1,"status":"running","operation":{"agent":"lead","epoch":1,"accountKey":"source","connectionId":"original"}})).to_string()])?;
            Ok(())
        })).expect("persist the unproven monitor");
        let transfer = "5a300f72-4ea6-4d30-8a0e-9b2c13c4a001";
        request(&store, transfer, "lead", "target", "team", "codex", 1.0)
            .expect("request the account transfer");
        let first = run_async(execute_member(&store, &NoNative, transfer, "lead", 2.0))
            .expect_err("unproven ownership blocks the transfer");
        assert!(
            first
                .to_string()
                .contains("verified source monitor ownership")
        );
        let before=run_async(store.transaction(|tx| {
            let operation=Repository::get(tx,transfer)?.expect("transfer remains");
            assert!(operation["members"]["lead"]["waiting"].as_str().expect("visible wait reason").contains("verified source monitor ownership"));
            assert_eq!(operation["members"]["lead"]["phase"],"lazy");
            tx.execute_batch("CREATE TRIGGER no_repeated_transfer_write BEFORE UPDATE ON runtime_account_transfers BEGIN SELECT RAISE(ABORT,'repeated transfer write'); END; CREATE TRIGGER no_repeated_actor_write BEFORE UPDATE ON runtime_agents BEGIN SELECT RAISE(ABORT,'repeated actor write'); END;")?;
            Ok(operation)
        })).expect("save the wait and install no-write guards");
        let second = run_async(execute_member(&store, &NoNative, transfer, "lead", 3.0))
            .expect_err("the same blocker remains read-only");
        assert_eq!(first.to_string(), second.to_string());
        let after = Repository::get(&store.test_reader().expect("reader"), transfer)
            .expect("read unchanged transfer")
            .expect("transfer remains");
        assert_eq!(
            before, after,
            "the same blocker changes no revision or timestamp"
        );
    }

    #[test]
    fn new_request_adopts_lazy_members_and_exact_replay_is_read_only() {
        let (_root, store) = store_with_agents();
        let id = "5a300f72-4ea6-4d30-8a0e-9b2c13c4226d";
        let first = request(&store, id, "lead", "target", "team", "codex", 10.0)
            .expect("test fixture operation succeeds");
        assert_eq!(first["id"], id);
        assert_eq!(first["targetAccountKey"], "target");
        assert_eq!(first["members"]["lead"]["phase"], "lazy");
        let lead = stored(&store, "lead");
        assert_eq!(lead["accountKey"], "target");
        assert_eq!(lead["lazyAccountTransfer"]["sourceAccountKey"], "source");
        let replay = request(&store, id, "lead", "target", "team", "codex", 11.0)
            .expect("test fixture operation succeeds");
        assert_eq!(replay, first);
    }

    #[test]
    fn exact_retry_returns_original_result_after_transfer_changes() {
        let (_root, store) = store_with_agents();
        let id = "5a300f72-4ea6-4d30-8a0e-9b2c13c4226b";
        let first = request(&store, id, "lead", "target", "team", "codex", 10.0).unwrap();
        let cancelled = run_async(action(&store, id, Action::Cancel, 20.0)).unwrap();
        assert_eq!(cancelled["status"], "cancelled");
        let replay = request(&store, id, "lead", "target", "team", "codex", 30.0).unwrap();
        assert_eq!(replay, first, "retry returns the original accepted result");
    }

    #[test]
    fn legacy_request_without_original_result_fails_closed() {
        let (_root, store) = store_with_agents();
        let id = "5a300f72-4ea6-4d30-8a0e-9b2c13c4226c";
        request(&store, id, "lead", "target", "team", "codex", 10.0).unwrap();
        run_async(store.transaction(move |tx| {
            let mut operation = Repository::get(tx, id)?.unwrap();
            operation["requests"][id]
                .as_object_mut()
                .unwrap()
                .remove("savedResponse");
            Repository::put(tx, id, &operation.to_string())
        }))
        .unwrap();
        let before = stored(&store, "lead");
        assert!(matches!(
            request(&store, id, "lead", "target", "team", "codex", 11.0),
            Err(TransferError::Refused(
                "The original transfer response is unavailable"
            ))
        ));
        assert_eq!(stored(&store, "lead"), before);
    }

    #[test]
    fn request_cancels_pending_usage_resume_in_the_transfer_transaction() {
        let (_root, store) = store_with_agents();
        let resume = json!({"id":"resume","status":"scheduled","accountKey":"source",
            "threadId":"source-thread","epoch":1,"turnId":"turn","cause":"rate_limit",
            "failedAt":1.0,"dueAt":200.0,"plannedAt":null,"resetAt":null,"reason":null,
            "taskClaims":[]});
        run_async(store.transaction_with_error(move |tx| {
            let mut lead = AgentRepository::get(tx, &AgentId::from("lead"))?.expect("lead exists");
            lead.extra.insert("usageResume".into(), resume.clone());
            lead.usage_resume_enabled = Some(true);
            AgentRepository::put(tx, &lead)?;
            tx.execute(
                "INSERT INTO runtime_usage_resumes(id,agent,record) VALUES(?1,?2,?3)",
                ("resume", "lead", resume.to_string()),
            )?;
            Ok::<_, studio_runtime_store::StoreError>(())
        }))
        .expect("fixture writes atomically");

        request(
            &store,
            "5a300f72-4ea6-4d30-8a0e-9b2c13c42279",
            "lead",
            "target",
            "team",
            "codex",
            10.0,
        )
        .expect("transfer request succeeds");

        let db = store.test_reader().expect("database is readable");
        let agent = AgentRepository::get(&db, &AgentId::from("lead"))
            .expect("lead read succeeds")
            .expect("lead exists")
            .value();
        assert_eq!(agent["usageResume"]["status"], "cancelled");
        assert_eq!(
            agent["usageResume"]["reason"],
            "The chat moved to another account."
        );
        assert_eq!(agent["usageResumeEnabled"], true);
        let saved: String = db
            .query_row(
                "SELECT record FROM runtime_usage_resumes WHERE id='resume'",
                [],
                |row| row.get(0),
            )
            .expect("resume row is readable");
        assert_eq!(
            serde_json::from_str::<Value>(&saved).expect("saved resume is valid JSON")["status"],
            "cancelled"
        );
    }

    fn set_lead_busy(store: &RuntimeStore, busy: bool) {
        run_async(store.transaction_with_error(move |tx| {
            let mut lead = AgentRepository::get(tx, &AgentId::from("lead"))?.expect("lead exists");
            lead.status = if busy {
                studio_runtime_store::AgentStatus::Running
            } else {
                studio_runtime_store::AgentStatus::Idle
            };
            lead.in_flight = Some(busy);
            AgentRepository::put(tx, &lead)?;
            Ok::<_, studio_runtime_store::StoreError>(())
        }))
        .expect("lead state is written");
    }

    fn stored(store: &RuntimeStore, id: &str) -> Value {
        AgentRepository::get(
            &store.test_reader().expect("database is readable"),
            &AgentId::from(id),
        )
        .expect("agent read succeeds")
        .expect("agent exists")
        .value()
    }

    #[test]
    fn busy_member_says_why_it_waits_and_moves_once_its_turn_ended() {
        let (_root, store) = store_with_agents();
        set_lead_busy(&store, true);
        let id = "5a300f72-4ea6-4d30-8a0e-9b2c13c4a101";
        let first = request(&store, id, "lead", "target", "team", "codex", 10.0)
            .expect("transfer request succeeds");
        assert_eq!(first["members"]["lead"]["phase"], "waiting");
        assert_eq!(
            first["members"]["lead"]["waiting"],
            crate::account_transfer::WAITING_FOR_NEXT_TURN
        );
        assert!(
            !run_async(release_waiting(&store, "lead", 11.0)).expect("release reads"),
            "a member stays while its turn runs"
        );
        assert_eq!(stored(&store, "lead")["accountKey"], "source");

        set_lead_busy(&store, false);
        assert!(run_async(release_waiting(&store, "lead", 12.0)).expect("release writes"));
        let lead = stored(&store, "lead");
        assert_eq!(lead["accountKey"], "target");
        assert_eq!(lead["accountTransferId"], id);
        assert_eq!(lead["lazyAccountTransfer"]["sourceAccountKey"], "source");
        assert_eq!(lead["accountTransfer"]["waitingCount"], 0);
        assert_eq!(lead["accountTransfer"]["nativeHistoryPending"], 1);
        let member = run_async(store.transaction_with_error(move |tx| {
            Ok::<_, TransferError>(load(tx, id)?.record().members.get("lead").unwrap().clone())
        }))
        .expect("transfer reads");
        assert_eq!(
            member.phase,
            studio_runtime_store::transfer_record::TransferPhase::Lazy
        );
        assert_eq!(member.lazy.value(), Some(&true));
        assert!(
            !run_async(release_waiting(&store, "lead", 13.0)).expect("release reads"),
            "a released member is not released again"
        );
    }

    #[test]
    fn exact_request_replays_after_target_becomes_excluded() {
        let (_root, store) = store_with_agents();
        let id = "5a300f72-4ea6-4d30-8a0e-9b2c13c4226a";
        let first = request(&store, id, "lead", "target", "team", "codex", 10.0)
            .expect("initial transfer succeeds");
        run_async(store.transaction_with_error(|tx| {
            let mut lead = AgentRepository::get(tx, &AgentId::from("lead"))?.unwrap();
            lead.extra
                .insert("excludedAccountKeys".into(), json!(["target"]));
            AgentRepository::put(tx, &lead)?;
            Ok::<_, studio_runtime_store::StoreError>(())
        }))
        .expect("change eligibility after the request");
        let before = stored(&store, "lead");
        assert_eq!(
            request(&store, id, "lead", "target", "team", "codex", 11.0)
                .expect("lost response retry replays the accepted request"),
            first
        );
        assert_eq!(stored(&store, "lead"), before);
    }

    #[test]
    fn invalid_scope_and_excluded_account_refuse_without_writes() {
        let (_root, store) = store_with_agents();
        let snapshot = || {
            let db = store.test_reader().expect("reader");
            let agents = ["lead", "child"].map(|id| stored(&store, id));
            let transfers = db
                .prepare("SELECT id, record FROM runtime_account_transfers ORDER BY id")
                .expect("prepare transfers")
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .expect("query transfers")
                .collect::<Result<Vec<_>, _>>()
                .expect("decode transfers");
            (agents, transfers)
        };
        let before = snapshot();
        assert!(
            request(
                &store,
                "5a300f72-4ea6-4d30-8a0e-9b2c13c4226e",
                "lead",
                "target",
                "bad",
                "codex",
                10.0
            )
            .is_err()
        );
        assert_eq!(snapshot(), before, "invalid scope wrote durable state");
        run_async(store.transaction_with_error(|tx| {
            let mut lead = AgentRepository::get(tx, &AgentId::from("lead"))?
                .expect("test fixture operation succeeds");
            lead.extra
                .insert("excludedAccountKeys".into(), json!(["blocked"]));
            AgentRepository::put(tx, &lead)?;
            Ok::<_, studio_runtime_store::StoreError>(())
        }))
        .expect("test fixture operation succeeds");
        let before_blocked = snapshot();
        assert!(
            request(
                &store,
                "5a300f72-4ea6-4d30-8a0e-9b2c13c4226f",
                "lead",
                "blocked",
                "team",
                "codex",
                10.0
            )
            .is_err()
        );
        assert_eq!(
            snapshot(),
            before_blocked,
            "excluded account wrote durable state"
        );
    }
}

#[cfg(test)]
mod provider_admission_tests {
    use super::*;
    use serde_json::json;

    fn snapshot(intent: bool, admission: Option<Value>) -> Snapshot {
        let mut operation = json!({"id":"transfer","leadId":"agent","targetAccountKey":"target",
            "status":"pending","created":1.0,"scope":"team",
            "members":{"agent":{"phase":"reading","sourceAccountKey":"source","sourceThreadId":"thread"}}});
        if intent {
            operation["members"]["agent"]["providerIntent"] = json!({"operation":"start",
                "scope":{"account":"target","connection":"connection","generation":1},
                "method":"thread/start","params":{}});
        }
        if let Some(admission) = admission {
            operation["members"]["agent"]["providerAdmission"] = admission;
        }
        Snapshot::read(&operation.to_string(),[("agent".into(), json!({"compactions":0,"provider":"codex","id":"agent",
            "rootId":"agent","epoch":0,"status":"idle","accountKey":"target","accountTransferId":"transfer"}).to_string())])
            .expect("transfer owner")
    }

    #[test]
    fn provider_root_is_persisted_once_and_missing_retry_identity_is_refused() {
        let first = snapshot(false, None);
        let mut decision = first.decide(Command::Restart).expect("decision");
        admit_mutation(&first, &mut decision, "agent").expect("new obligation");
        let saved = serde_json::to_value(
            decision
                .operation
                .members
                .get("agent")
                .unwrap()
                .provider_admission
                .value()
                .unwrap(),
        )
        .unwrap();
        let admission: ProviderAdmission =
            serde_json::from_value(saved.clone()).expect("admission");
        assert!(admission.admitted_request.value().is_none());
        studio_runtime_schema::persistence::RequestIdentity::new(
            "test",
            &[],
            &admission.root_request_id,
            &[],
        )
        .expect("v7 root");
        let retry = snapshot(true, Some(saved.clone()));
        let mut decision = retry.decide(Command::Restart).expect("retry decision");
        admit_mutation(&retry, &mut decision, "agent").expect("saved identity");
        assert_eq!(
            serde_json::to_value(
                decision
                    .operation
                    .members
                    .get("agent")
                    .unwrap()
                    .provider_admission
                    .value()
                    .unwrap()
            )
            .unwrap(),
            saved
        );
        let missing = snapshot(true, None);
        let mut decision = missing.decide(Command::Restart).expect("held decision");
        assert!(admit_mutation(&missing, &mut decision, "agent").is_err());
        assert!(
            decision
                .operation
                .members
                .get("agent")
                .unwrap()
                .provider_admission
                .value()
                .is_none()
        );
    }
}
