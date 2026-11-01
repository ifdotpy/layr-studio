//! Pure account-transfer decisions. Stored JSON enters only through `Snapshot::read`.
//!
//! Submission is durable before an effect is dispatched. Unknown outcomes never
//! authorize another send; receipts retain their exact transport identity.
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use thiserror::Error;

use crate::account_session_runtime::{
    ConnectionScope, ProviderResponse, RequestCertainty, SessionReceipt,
};
use studio_runtime_schema::agent_limits::MAX_TRANSFER_AGENT_RECORDS;
pub use studio_runtime_store::MAX_TRANSFER_RECORD_BYTES as MAX_RECORD_BYTES;
pub const EXECUTOR_REQUIRED: &str = "account-transfer/provider-history-executor";
pub const SETTINGS_REQUIRED: &str = "account-transfer/destination-settings";
/// Internal receipt-hold identity mapped to native admission by preparation.
pub(crate) const RECEIPT_HOLD: &str = "account-transfer/receipt-hold";

/// A retained receipt or a failure before a receipt could be obtained.
pub enum TransferDispatch {
    Receipt(SessionReceipt),
    Unknown {
        operation: String,
        scope: ConnectionScope,
    },
}

impl TransferDispatch {
    fn identity(&self) -> (&str, &ConnectionScope) {
        match self {
            Self::Receipt(receipt) => (&receipt.operation, &receipt.scope),
            Self::Unknown { operation, scope } => (operation, scope),
        }
    }

    fn certainty(&self) -> RequestCertainty {
        match self {
            Self::Receipt(receipt) => receipt.certainty(),
            Self::Unknown { .. } => RequestCertainty::Unknown,
        }
    }

    fn mutation_result(&self) -> Option<Value> {
        match self {
            Self::Receipt(receipt) => receipt.response().map(|response| match response {
                ProviderResponse::Reply(value) => value.clone(),
                ProviderResponse::Rejected(value) => json!({"rejected":value}),
            }),
            Self::Unknown { .. } => None,
        }
    }

    pub(crate) fn has_thread(&self) -> bool {
        matches!(self, Self::Receipt(receipt) if matches!(receipt.response(),
            Some(ProviderResponse::Reply(value)) if value.pointer("/thread/id").and_then(Value::as_str).is_some_and(|id| !id.is_empty())))
    }

    fn mutation_record(
        &self,
        result: &Option<Value>,
    ) -> Result<studio_runtime_store::transfer_record::ProviderReceipt, TransferError> {
        use studio_runtime_schema::queue_command::Field;
        use studio_runtime_store::transfer_record::ProviderReceipt;
        match self {
            Self::Receipt(receipt) => {
                let result = match result {
                    Some(value) if !value.is_null() => Field::Value(
                        serde_json::value::to_raw_value(value)
                            .map_err(|_| invalid("provider payload", "/"))?,
                    ),
                    Some(_) | None => Field::Null,
                };
                Ok(ProviderReceipt {
                    operation: receipt.operation.clone(),
                    scope: receipt.scope.clone(),
                    certainty: receipt_certainty(receipt.certainty()),
                    ticket: provider_ticket(receipt)?,
                    not_sent_reason: receipt
                        .not_sent_reason()
                        .map_or(Field::Null, |reason| Field::Value(format!("{reason:?}"))),
                    result,
                })
            }
            Self::Unknown { operation, scope } => Ok(ProviderReceipt {
                operation: operation.clone(),
                scope: scope.clone(),
                certainty: receipt_certainty(self.certainty()),
                ticket: Field::Null,
                not_sent_reason: Field::Missing,
                result: Field::Missing,
            }),
        }
    }
}

fn receipt_certainty(
    certainty: RequestCertainty,
) -> studio_runtime_store::transfer_record::ReceiptCertainty {
    use studio_runtime_store::transfer_record::ReceiptCertainty;
    match certainty {
        RequestCertainty::Written => ReceiptCertainty::Written,
        RequestCertainty::NotSent => ReceiptCertainty::NotSent,
        RequestCertainty::Unknown => ReceiptCertainty::Unknown,
    }
}

fn provider_ticket(
    receipt: &SessionReceipt,
) -> Result<
    studio_runtime_schema::queue_command::Field<
        studio_runtime_store::transfer_record::ProviderTicket,
    >,
    TransferError,
> {
    use studio_runtime_schema::queue_command::Field;
    use studio_runtime_store::transfer_record::ProviderTicket;
    let Some(ticket) = receipt.ticket() else {
        return Ok(Field::Null);
    };
    let method = serde_json::from_str(
        &serde_json::to_string(ticket.method.as_str())
            .map_err(|_| invalid("provider ticket", "/method"))?,
    )
    .map_err(|_| invalid("provider ticket", "/method"))?;
    Ok(Field::Value(ProviderTicket {
        generation: ticket.generation,
        id: ticket.id.clone(),
        method,
    }))
}

type ReadResult = (
    studio_runtime_schema::queue_command::Field<
        studio_runtime_store::transfer_record::ProviderResult,
    >,
    Option<(String, Vec<u8>)>,
);
fn read_result(response: Option<&ProviderResponse>) -> Result<ReadResult, TransferError> {
    use studio_runtime_schema::queue_command::Field;
    use studio_runtime_store::transfer_record::{PayloadBlob, ProviderResult, ResponseKind};
    let (kind, value) = match response {
        Some(ProviderResponse::Reply(value)) => (ResponseKind::Reply, value),
        Some(ProviderResponse::Rejected(value)) => (ResponseKind::Rejected, value),
        None => return Ok((Field::Null, None)),
    };
    let bytes = serde_json::to_vec(value).map_err(|_| invalid("provider payload", "/"))?;
    let (body, spill) = if bytes.len() > studio_runtime_store::MAX_INLINE_READ_REPLY_BYTES {
        let digest = studio_runtime_store::sha256_hex(&bytes);
        let blob = PayloadBlob {
            sha256: digest.clone(),
            size: bytes.len() as u64,
        };
        (
            ProviderResult::Blob {
                kind,
                value_blob: blob,
            },
            Some((digest, bytes)),
        )
    } else if value.is_null() {
        (ProviderResult::Inline { kind, value: None }, None)
    } else {
        (
            ProviderResult::Inline {
                kind,
                value: Some(
                    serde_json::value::to_raw_value(value)
                        .map_err(|_| invalid("provider payload", "/"))?,
                ),
            },
            None,
        )
    };
    Ok((Field::Value(body), spill))
}

/// The reply body that `provider_read_receipt` leaves out of the record.
pub(crate) fn spilled_reply(receipt: &SessionReceipt) -> Option<(String, Vec<u8>)> {
    read_result(receipt.response()).ok()?.1
}

/// Exact source preflight shapes. A background read checks one bounded page;
/// a nonempty result or continuation cursor keeps the transfer on its source.
pub fn valid_source_preflight_params(method: &str, thread: &str, params: &Value) -> bool {
    if thread.is_empty() || params.get("threadId").and_then(Value::as_str) != Some(thread) {
        return false;
    }
    match method {
        "thread/queue/list" | "thread/unsubscribe" => params == &json!({"threadId":thread}),
        "thread/backgroundTerminals/list" => {
            params
                == &json!({
                    "threadId":thread,
                    "limit":studio_runtime_schema::shared_values::NATIVE_TERMINAL_ITEMS_PER_PAGE,
                })
        }
        _ => false,
    }
}

#[derive(Debug, Error)]
pub enum TransferError {
    #[error("Request id expired")]
    Expired,
    #[error("invalid stored {object} at {pointer}")]
    Stored {
        object: &'static str,
        pointer: String,
    },
    #[error("{0}")]
    Refused(&'static str),
    #[error("Not implemented: requires {0}")]
    Requires(&'static str),
    #[error(transparent)]
    Store(#[from] studio_runtime_store::StoreError),
}

impl From<studio_runtime_store::transfer_record::TransferRecordError> for TransferError {
    fn from(error: studio_runtime_store::transfer_record::TransferRecordError) -> Self {
        use studio_runtime_store::transfer_record::TransferRecordError;
        match error {
            TransferRecordError::ConflictingRequest => {
                Self::Refused("This transfer id has different content")
            }
            TransferRecordError::UnknownRequest => {
                Self::Refused("The original transfer request is unavailable")
            }
        }
    }
}

pub use studio_runtime_store::transfer_record::TransferPhase as Phase;
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Cancel,
    Retry,
    FinishHistory,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Restart,
    Action(Action),
}

pub use studio_runtime_store::transfer_record::TransportTicket as Ticket;

use studio_runtime_store::transfer_record::TransferStatus;

/// Project the stored summary into the closed renderer progress contract.
pub fn progress_view(
    summary: Option<&studio_runtime_store::transfer_record::AgentTransferState>,
    can_cancel: bool,
) -> studio_runtime_schema::agent_progress::AccountTransferProgress {
    use studio_runtime_schema::agent_progress::{
        AccountTransferActions as Actions, AccountTransferProgress as Progress,
        MovingTransferActions, WaitingTransferActions,
    };
    let Some(summary) = summary else {
        return Progress::None;
    };
    let summary = match summary {
        studio_runtime_store::transfer_record::AgentTransferState::Unreadable(value) => {
            return Progress::Unreadable {
                reason: value.reason.clone(),
            };
        }
        studio_runtime_store::transfer_record::AgentTransferState::Summary(value) => value,
    };
    let id = summary.id.as_str().to_owned();
    let target_account_key = summary.target_account_key.clone();
    let status = &summary.status;
    let waiting = summary.waiting.value().cloned();
    let can_retry = summary.can_retry.value() == Some(&true);
    match status {
        TransferStatus::Completed => Progress::Completed {
            id,
            target_account_key,
        },
        TransferStatus::Cancelled => Progress::Cancelled {
            id,
            target_account_key,
        },
        TransferStatus::Pending => {
            let actions = match (can_retry, can_cancel) {
                (false, false) => Actions::None,
                (true, false) => Actions::Retry,
                (false, true) => Actions::Cancel,
                (true, true) => Actions::RetryCancel,
            };
            if let Some(block) = summary.blocked.value().and_then(|blocked| blocked.first()) {
                Progress::Blocked {
                    id,
                    target_account_key,
                    reason: block
                        .reason
                        .value()
                        .cloned()
                        .or(waiting)
                        .unwrap_or_default(),
                    actions,
                }
            } else if summary
                .moving_now
                .value()
                .is_some_and(|count| count.as_i64() > 0)
            {
                Progress::Moving {
                    id,
                    target_account_key,
                    actions: MovingTransferActions::None,
                }
            } else {
                Progress::Waiting {
                    id,
                    target_account_key,
                    reason: waiting.unwrap_or_default(),
                    actions: if can_cancel {
                        WaitingTransferActions::Cancel
                    } else {
                        WaitingTransferActions::None
                    },
                }
            }
        }
    }
}

/// Validated snapshot with original member insertion order retained for the UI.
pub struct Snapshot {
    operation: studio_runtime_store::transfer_record::TransferRecord,
    agents: BTreeMap<String, studio_runtime_store::AgentRecord>,
}
#[derive(Clone)]
pub struct Followup {
    pub agent: String,
    pub id: String,
    pub text: &'static str,
}
pub struct Decision {
    pub operation: studio_runtime_store::transfer_record::TransferRecord,
    pub agents: BTreeMap<String, studio_runtime_store::AgentRecord>,
    pub followups: Vec<Followup>,
    pub changed: bool,
}

fn invalid(object: &'static str, pointer: impl Into<String>) -> TransferError {
    TransferError::Stored {
        object,
        pointer: pointer.into(),
    }
}
fn validate(schema: &str, object: &'static str, value: &Value) -> Result<(), TransferError> {
    if let Some((pointer, _)) =
        studio_runtime_schema::first_error(schema, value).map_err(|_| invalid(object, "/"))?
    {
        return Err(invalid(object, pointer));
    }
    Ok(())
}
pub(crate) fn read_record(
    raw: &str,
    schema: &str,
    object: &'static str,
) -> Result<Value, TransferError> {
    if raw.len() > MAX_RECORD_BYTES {
        return Err(invalid(object, "/resourceLimit"));
    }
    let value: Value = serde_json::from_str(raw).map_err(|_| invalid(object, "/"))?;
    validate(schema, object, &value)?;
    Ok(value)
}

impl Snapshot {
    pub fn read(
        raw: &str,
        agents: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, TransferError> {
        Self::read_fallible(raw, agents.into_iter().map(Ok))
    }

    /// Consume store reads lazily so resource limits stop the next record read.
    pub(crate) fn read_fallible(
        raw: &str,
        agents: impl IntoIterator<Item = Result<(String, String), TransferError>>,
    ) -> Result<Self, TransferError> {
        if raw.len() > MAX_RECORD_BYTES {
            return Err(invalid("account transfer", "/resourceLimit"));
        }
        let operation = serde_json::from_str(raw).map_err(|_| invalid("account transfer", "/"))?;
        Self::read_typed(operation, raw, agents)
    }

    /// The store already decoded this operation; keep its ordered members.
    pub(crate) fn read_typed(
        record: studio_runtime_store::transfer_record::TransferRecord,
        raw: &str,
        agents: impl IntoIterator<Item = Result<(String, String), TransferError>>,
    ) -> Result<Self, TransferError> {
        read_record(raw, "PersistedAccountTransferRecord", "account transfer")?;
        Self::from_parts(record, raw.len(), agents)
    }

    fn from_parts(
        operation: studio_runtime_store::transfer_record::TransferRecord,
        raw_len: usize,
        agents: impl IntoIterator<Item = Result<(String, String), TransferError>>,
    ) -> Result<Self, TransferError> {
        let mut records = BTreeMap::new();
        let mut bytes = raw_len;
        for record in agents {
            let (id, raw) = record?;
            bytes = bytes.saturating_add(raw.len());
            if records.len() >= MAX_TRANSFER_AGENT_RECORDS
                || bytes > studio_runtime_store::AccountTransferRepository::MAX_SNAPSHOT_BYTES
            {
                return Err(invalid("transfer snapshot", "/resourceLimit"));
            }
            read_record(&raw, "PersistedAgentRecord", "transfer agent")?;
            let agent: studio_runtime_store::AgentRecord =
                serde_json::from_str(&raw).map_err(|_| invalid("transfer agent", "/"))?;
            if agent.id.0 != id || records.insert(id, agent).is_some() {
                return Err(invalid("transfer agent", "/id"));
            }
        }
        for id in operation
            .members
            .0
            .iter()
            .map(|(id, _)| id.as_str())
            .chain(std::iter::once(operation.lead_id.as_str()))
        {
            if !records.contains_key(id) {
                return Err(invalid("transfer agent", "/id"));
            }
        }
        Ok(Self {
            operation,
            agents: records,
        })
    }
    pub fn typed_public_view(
        &self,
    ) -> studio_runtime_store::transfer_record::SavedTransferResponse {
        public_view_record(&self.operation)
    }
    pub fn lead_id(&self) -> &str {
        &self.operation.lead_id
    }
    pub(crate) fn record(&self) -> &studio_runtime_store::transfer_record::TransferRecord {
        &self.operation
    }
    pub(crate) fn add_team_request(
        mut self,
        id: String,
        lead: String,
        target: studio_runtime_schema::account_key::AccountKey,
    ) -> Result<Decision, TransferError> {
        self.operation.add_team_request(id, lead, target)?;
        Ok(Decision {
            operation: self.operation,
            agents: self.agents,
            followups: Vec::new(),
            changed: true,
        })
    }
    pub(crate) fn can_cancel(&self) -> bool {
        self.pending() && check_cancel(&self.operation, &self.agents).is_ok()
    }
    pub fn agent(&self, id: &str) -> Result<&studio_runtime_store::AgentRecord, TransferError> {
        self.agents
            .get(id)
            .ok_or(TransferError::Refused("Unknown transfer member"))
    }
    fn phase(&self, id: &str) -> Result<Phase, TransferError> {
        self.operation
            .members
            .get(id)
            .map(|member| member.phase)
            .ok_or_else(|| invalid("account transfer", "/members/phase"))
    }
    fn pending(&self) -> bool {
        self.operation.status == TransferStatus::Pending
    }
    fn decision(
        &self,
        operation: studio_runtime_store::transfer_record::TransferRecord,
        agents: BTreeMap<String, studio_runtime_store::AgentRecord>,
        followups: Vec<Followup>,
    ) -> Decision {
        let changed = serde_json::to_vec(&operation).ok()
            != serde_json::to_vec(&self.operation).ok()
            || agents != self.agents
            || !followups.is_empty();
        Decision {
            operation,
            agents,
            followups,
            changed,
        }
    }
    /// A returned operation is bound to its original request content, even after completion.
    pub fn replay(
        &self,
        request: &str,
        lead: &str,
        target: &str,
        scope: &str,
    ) -> Result<bool, TransferError> {
        use studio_runtime_store::transfer_record::TransferScope;
        let receipt = self
            .operation
            .requests
            .value()
            .and_then(|requests| requests.get(request));
        let (saved_lead, saved_target, saved_scope) = if let Some(receipt) = receipt {
            (
                &receipt.lead_id,
                &receipt.target_account_key,
                &receipt.scope,
            )
        } else if self.operation.id == request {
            (
                &self.operation.lead_id,
                &self.operation.target_account_key,
                &self.operation.scope,
            )
        } else {
            return Ok(false);
        };
        let saved_scope = match saved_scope.value() {
            Some(TransferScope::Subagents) => "subagents",
            Some(TransferScope::Team) | None => "team",
        };
        if saved_lead != lead || saved_target.as_str() != target || saved_scope != scope {
            return Err(TransferError::Refused(
                "This transfer id has different content",
            ));
        }
        Ok(true)
    }
    pub fn decide(&self, command: Command) -> Result<Decision, TransferError> {
        use studio_runtime_schema::queue_command::Field;
        use studio_runtime_store::transfer_record::InterruptOutcome;
        let mut op = self.operation.clone();
        let mut agents = self.agents.clone();
        let mut followups = Vec::new();
        if !self.pending() {
            return Ok(self.decision(self.operation.clone(), agents, followups));
        }
        match command {
            Command::Restart => {
                for (_, member) in &mut op.members.0 {
                    match member.phase {
                        Phase::Reading => {
                            member.phase = if member.lazy.value() == Some(&true) {
                                Phase::Lazy
                            } else {
                                Phase::Waiting
                            };
                            member.waiting = Field::Value(WAITING_FOR_NEXT_TURN.into());
                        }
                        Phase::Submitted | Phase::LazySubmitted => {
                            member.phase = Phase::Unknown;
                            member.error = Field::Value("The server restarted before the transfer receipt arrived. No request was repeated.".into());
                        }
                        Phase::Interrupting => {
                            member.phase = Phase::Blocked;
                            member.interrupt_outcome = Field::Value(InterruptOutcome::Unknown);
                            member.error = Field::Value("The server restarted before the turn interrupt receipt arrived. No request was repeated.".into());
                        }
                        Phase::Waiting
                        | Phase::Lazy
                        | Phase::Ready
                        | Phase::Blocked
                        | Phase::Unknown
                        | Phase::Completed
                        | Phase::Left => {}
                    }
                }
            }
            Command::Action(Action::FinishHistory) => op.finish_history = Field::Value(true),
            Command::Action(Action::Retry) => {
                if op
                    .members
                    .0
                    .iter()
                    .any(|(_, member)| member.archive_invalidated.value() == Some(&true))
                {
                    return Err(TransferError::Refused(
                        "The source changed after history export. Cancel this transfer and start a new one.",
                    ));
                }
                for (id, member) in &mut op.members.0 {
                    if member.phase != Phase::Blocked {
                        continue;
                    }
                    if member.lazy.value() == Some(&true)
                        && !agents[id].lazy_account_transfer.is_some()
                    {
                        return Err(TransferError::Requires(SETTINGS_REQUIRED));
                    }
                    let result = payload_field_is_truthy(&member.result);
                    member.phase = if member.lazy.value() == Some(&true) {
                        Phase::Lazy
                    } else if result {
                        Phase::Ready
                    } else {
                        Phase::Waiting
                    };
                    if member.phase != Phase::Ready {
                        member.waiting = Field::Value(WAITING_FOR_NEXT_TURN.into());
                    }
                    member.error = Field::Null;
                    member.next_check = Field::Value(0.into());
                    if numeric_field_is_nonzero(&member.interrupt_submitted_at) {
                        member.interrupt_turn_id = Field::Missing;
                        member.interrupt_submitted_at = Field::Missing;
                        member.interrupt_outcome = Field::Missing;
                    }
                }
            }
            Command::Action(Action::Cancel) => {
                check_cancel(&self.operation, &agents)?;
                op.status = TransferStatus::Cancelled;
                for (id, m) in &mut op.members.0 {
                    let a = agents
                        .get_mut(id)
                        .ok_or_else(|| invalid("transfer agent", "/id"))?;
                    if a.account_transfer_id.value().map(|id| id.as_str()) != Some(op.id.as_str())
                        || m.phase == Phase::Reading
                    {
                        continue;
                    }
                    // An unreadable lazy pointer keeps its hold because its source account is unknown.
                    let lazy = a
                        .lazy_account_transfer
                        .value()
                        .and_then(|state| state.pending())
                        .cloned();
                    if let Some(lazy) = lazy
                        .filter(|lazy| lazy.id.as_str() == op.id && m.lazy.value() == Some(&true))
                    {
                        a.account_key = lazy.source_account_key.as_str().to_owned();
                        let mut unreadable = false;
                        let exact = match &lazy.source_state_exact {
                            Field::Missing => false,
                            Field::Value(value) => *value,
                            Field::Null => {
                                unreadable = true;
                                false
                            }
                        };
                        if exact && !lazy.source_state.is_some() {
                            unreadable = true;
                        }
                        if let Some(source) = lazy.source_state.value() {
                            if (exact
                                && (!source.account_key.is_some() || !source.status.is_some()))
                                || matches!(source.account_key, Field::Null)
                                || matches!(source.status, Field::Null)
                            {
                                unreadable = true;
                            }
                            if let Some(account) = source.account_key.value() {
                                a.account_key = account.as_str().to_owned();
                            }
                            restore_source_field(&source.provider, &mut a.provider, exact);
                            restore_optional(&source.model, &mut a.model, exact);
                            restore_source_field(&source.effort, &mut a.effort, exact);
                            restore_source_field(
                                &source.native_effort,
                                &mut a.native_effort,
                                exact,
                            );
                            restore_optional(&source.fast_mode, &mut a.fast_mode, exact);
                            restore_optional(
                                &source.daybreak_enabled,
                                &mut a.daybreak_enabled,
                                exact,
                            );
                            let cyber = source
                                .cyber_access_program
                                .clone()
                                .map(studio_runtime_store::StoredTransferValue::Known);
                            restore_source_field(&cyber, &mut a.cyber_access_program, exact);
                            let defaults = source
                                .worker_defaults
                                .clone()
                                .map(studio_runtime_store::StoredWorkerDefaults::Known);
                            restore_source_field(&defaults, &mut a.worker_defaults, exact);
                            let claude_options = source
                                .claude_options
                                .clone()
                                .map(studio_runtime_store::StoredTransferObject::Known);
                            restore_source_field(&claude_options, &mut a.claude_options, exact);
                            let pending_account = source
                                .pending_settings_account_key
                                .clone()
                                .map(studio_runtime_store::StoredTransferValue::Known);
                            restore_source_field(
                                &pending_account,
                                &mut a.pending_settings_account_key,
                                exact,
                            );
                            let pending = source.pending_settings.clone().map(|value| {
                                studio_runtime_store::StoredTransferObject::Known(Box::new(value))
                            });
                            restore_source_field(&pending, &mut a.pending_settings, exact);
                            let execution_account = source
                                .execution_settings_account_key
                                .clone()
                                .map(|key| key.as_str().to_owned());
                            restore_source_field(
                                &execution_account,
                                &mut a.execution_settings_account_key,
                                exact,
                            );
                            let error = source
                                .error
                                .clone()
                                .map(studio_runtime_schema::provider_errors::AgentError::Message);
                            restore_source_field(&error, &mut a.error, exact);
                            let hold = source
                                .native_failure_hold
                                .clone()
                                .map(studio_runtime_store::NativeFailureHold::Boolean);
                            restore_source_field(&hold, &mut a.native_failure_hold, exact);
                            if let Some(status) = source.status.value() {
                                a.status = status.clone();
                            }
                        }
                        if unreadable {
                            m.error = Field::Value("The saved transfer source is incomplete; its original state was retained".into());
                            let original = serde::Serialize::serialize(
                                &(&lazy),
                                serde_json::value::Serializer,
                            )
                            .map_err(|_| invalid("transfer agent", "/lazyAccountTransfer"))?;
                            append_history(
                                a,
                                studio_runtime_store::StoredTransferObject::Unreadable(original),
                            );
                        }
                        a.lazy_account_transfer = Field::Missing;
                    }
                    if m.interrupt_reason
                        .value()
                        .is_some_and(|reason| !reason.is_empty())
                        && m.continue_after_transfer.value() == Some(&true)
                        && a.auto_wake == Some(true)
                    {
                        followups.push(Followup {
                            agent: id.clone(),
                            id: format!("account-transfer-cancel:{}:{id}", op.id),
                            text: CANCEL_FOLLOWUP,
                        });
                        a.status = studio_runtime_store::AgentStatus::Queued;
                    }
                    a.account_transfer_id = Field::Missing;
                }
            }
        }
        let mut decision = self.decision(op, agents, followups);
        decision.changed |= matches!(command, Command::Action(_));
        Ok(decision)
    }

    /// Publish a durable pre-move blocker without changing process or input ownership.
    pub(crate) fn waiting_for_blocker(
        &self,
        agent: &str,
        reason: &str,
    ) -> Result<Decision, TransferError> {
        use studio_runtime_schema::queue_command::Field;
        let mut operation = self.operation.clone();
        if self.pending() && matches!(self.phase(agent)?, Phase::Lazy | Phase::Waiting) {
            operation
                .members
                .get_mut(agent)
                .ok_or(TransferError::Refused("Unknown transfer member"))?
                .waiting = Field::Value(reason.into());
        }
        Ok(self.decision(operation, self.agents.clone(), vec![]))
    }

    /// A member busy with a turn at the request waits. Once that turn has ended
    /// it is enrolled again as an idle member: the lazy hold moves its account,
    /// and its history moves at its next turn start. Saved provider journals
    /// stay on the member. `None` when the member cannot be released now.
    pub fn release_waiting(&self, agent: &str) -> Result<Option<Decision>, TransferError> {
        let Some(current) = self.agents.get(agent) else {
            return Ok(None);
        };
        let Some(member) = self.operation.members.get(agent) else {
            return Ok(None);
        };
        let submitted = numeric_field_is_nonzero(&member.submitted_at);
        let result = payload_field_is_truthy(&member.result);
        if !self.pending()
            || self.phase(agent)? != Phase::Waiting
            || member.lazy.value() == Some(&true)
            || current.account_transfer_id.value().map(|id| id.as_str())
                != Some(self.operation.id.as_str())
            || current.lazy_account_transfer.is_some()
            || current.is_deleted()
            || current.in_flight == Some(true)
            || matches!(
                current.status,
                studio_runtime_store::AgentStatus::Running
                    | studio_runtime_store::AgentStatus::Starting
                    | studio_runtime_store::AgentStatus::Approval
            )
            || submitted
            || result
            || member.transport_ticket.is_some()
            || member.provider_intent.is_some()
        {
            return Ok(None);
        }
        let target_provider = self
            .operation
            .target_provider
            .value()
            .copied()
            .ok_or_else(|| invalid("account transfer", "/targetProvider"))?;
        let mut agents = self.agents.clone();
        let value = agents
            .get_mut(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        let id = self
            .operation
            .id
            .clone()
            .try_into()
            .map_err(|_| invalid("account transfer", "/id"))?;
        let enrolled = typed_enroll(
            value,
            &id,
            &self.operation.target_account_key,
            target_provider,
            agent == self.operation.lead_id,
        )?;
        if enrolled.phase != Phase::Lazy {
            // Completed or blocked members hold no transfer pointer.
            value.account_transfer_id = studio_runtime_schema::queue_command::Field::Missing;
        }
        let mut op = self.operation.clone();
        let saved = op
            .members
            .get_mut(agent)
            .ok_or_else(|| invalid("account transfer", "/members"))?;
        saved.phase = enrolled.phase;
        saved.source_account_key = enrolled.source_account_key;
        saved.source_thread_id = enrolled.source_thread_id;
        saved.provider = enrolled.provider;
        saved.name = enrolled.name;
        if !enrolled.reason.is_missing() {
            saved.reason = enrolled.reason;
        }
        if !enrolled.lazy.is_missing() {
            saved.lazy = enrolled.lazy;
        }
        if !enrolled.pending_settings.is_missing() {
            saved.pending_settings = enrolled.pending_settings;
        }
        if !enrolled.source_pending_settings.is_missing() {
            saved.source_pending_settings = enrolled.source_pending_settings;
        }
        if !enrolled.source_claude_options.is_missing() {
            saved.source_claude_options = enrolled.source_claude_options;
        }
        if !enrolled.error.is_missing() {
            saved.error = enrolled.error;
        }
        saved.waiting = enrolled.waiting;
        if op
            .members
            .0
            .iter()
            .all(|(_, member)| member.phase.terminal())
        {
            op.status = TransferStatus::Completed;
        }
        Ok(Some(self.decision(op, agents, vec![])))
    }

    pub fn begin_read(&self, agent: &str) -> Result<Decision, TransferError> {
        use studio_runtime_schema::queue_command::Field;
        let a = self
            .agents
            .get(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        let member = self
            .operation
            .members
            .get(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        if !self.pending()
            || !matches!(self.phase(agent)?, Phase::Waiting | Phase::Lazy)
            || a.account_transfer_id.value().map(|id| id.as_str())
                != Some(self.operation.id.as_str())
            || a.is_deleted()
            || payload_field_is_truthy(&member.result)
            || numeric_field_is_nonzero(&member.submitted_at)
            || member.archive_invalidated.value() == Some(&true)
            || a.in_flight == Some(true)
            || matches!(
                a.status,
                studio_runtime_store::AgentStatus::Running
                    | studio_runtime_store::AgentStatus::Starting
                    | studio_runtime_store::AgentStatus::Approval
            )
        {
            return Err(TransferError::Refused(
                "The transfer is not ready for history preparation",
            ));
        }
        let mut op = self.operation.clone();
        let member = op
            .members
            .get_mut(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        member.phase = Phase::Reading;
        member.source = Field::Value(studio_runtime_store::transfer_record::TransferSource {
            epoch: Field::Value(a.epoch),
            account_key: Field::Value(
                a.account_key
                    .clone()
                    .try_into()
                    .map_err(|_| invalid("transfer agent", "/accountKey"))?,
            ),
            thread_id: a.thread_id.clone().map_or(Field::Null, Field::Value),
            cwd: a.cwd.clone().map_or(Field::Null, Field::Value),
        });
        member.settings = Field::Value(settings_snapshot(a)?);
        member.pending_settings = match known_transfer_object(
            &a.pending_settings,
            "pendingSettings",
        )?
        .map(|value| *value)
        {
            Field::Missing => Field::Null,
            value @ (Field::Null | Field::Value(_)) => value,
        };
        member.pending_settings_account_key = match known_transfer_value(
            &a.pending_settings_account_key,
            "pendingSettingsAccountKey",
        )? {
            Field::Missing => Field::Null,
            value @ (Field::Null | Field::Value(_)) => value,
        };
        member.error = Field::Null;
        // The move has started; the card shows it as moving now.
        member.waiting = Field::Missing;
        Ok(self.decision(op, self.agents.clone(), vec![]))
    }

    /// Persist the stable provider operation and current connection scope before
    /// crossing the retained-session boundary. A changed replay is refused.
    pub fn typed_provider_intent(
        &self,
        agent: &str,
        operation: &str,
        scope: studio_runtime_schema::provider_call_contract::ConnectionScope,
        method: studio_runtime_store::transfer_record::TransferMethod,
        params: &serde_json::value::RawValue,
    ) -> Result<Decision, TransferError> {
        use studio_runtime_schema::queue_command::Field;
        use studio_runtime_store::transfer_record::{ProviderIntent, TransferMethod};
        let member = self
            .operation
            .members
            .get(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        let params_value: Value =
            serde_json::from_str(params.get()).map_err(|_| invalid("provider payload", "/"))?;
        if !self.pending()
            || member.phase != Phase::Reading
            || operation.is_empty()
            || !matches!(method, TransferMethod::Start | TransferMethod::Fork)
            || !params_value.is_object()
        {
            return Err(TransferError::Refused(
                "The transfer is not ready for provider submission",
            ));
        }
        validate_source_for_submission(member, self.agent(agent)?)?;
        let intent = ProviderIntent {
            operation: operation.into(),
            account: Field::Missing,
            scope,
            method,
            params: params.to_owned(),
        };
        if let Some(saved) = member.provider_intent.value() {
            let saved_params: Value = serde_json::from_str(saved.params.get())
                .map_err(|_| invalid("provider payload", "/"))?;
            if !(saved.operation == intent.operation
                && saved.account.is_missing()
                && saved.scope == intent.scope
                && saved.method == intent.method
                && saved_params == params_value)
            {
                return Err(TransferError::Refused(
                    "The transfer provider intent changed; inspect its receipt",
                ));
            }
        } else if !member.provider_intent.is_missing() {
            return Err(TransferError::Refused(
                "The transfer provider intent changed; inspect its receipt",
            ));
        }
        let mut op = self.operation.clone();
        let member = op
            .members
            .get_mut(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        member.phase = if member.lazy.value() == Some(&true) {
            Phase::LazySubmitted
        } else {
            Phase::Submitted
        };
        member.native_method = Field::Value(intent.method.clone());
        member.native_params = Field::Value(intent.params.clone());
        if member.provider_intent.is_missing() {
            member.provider_intent = Field::Value(intent);
        }
        Ok(self.decision(op, self.agents.clone(), vec![]))
    }

    /// Replay an exact read or source-unsubscribe reply with its original scope.
    /// A new authenticated connection does not authorize another native send.
    pub fn written_read_replay_typed(
        &self,
        agent: &str,
        next: &studio_runtime_store::transfer_record::ProviderIntent,
    ) -> Result<Option<studio_runtime_store::transfer_record::ProviderReadReceipt>, TransferError>
    {
        use studio_runtime_store::transfer_record::{
            ReceiptCertainty, ResponseKind, TransferMethod,
        };
        if !matches!(
            next.method,
            TransferMethod::ModelList
                | TransferMethod::ThreadRead
                | TransferMethod::TurnsList
                | TransferMethod::QueueList
                | TransferMethod::BackgroundList
                | TransferMethod::Unsubscribe
        ) {
            return Ok(None);
        }
        let Some(member) = self.operation.members.get(agent) else {
            return Ok(None);
        };
        let Some(saved) = member
            .provider_read_intents
            .value()
            .and_then(|intents| intents.get(&next.operation))
        else {
            return Ok(None);
        };
        let Some(result) = member
            .provider_read_receipts
            .value()
            .and_then(|receipts| receipts.get(&next.operation))
        else {
            return Ok(None);
        };
        if result.receipt.certainty != ReceiptCertainty::Written {
            return Ok(None);
        }
        let refused = || {
            TransferError::Refused("The Written read receipt does not match its original intent")
        };
        let account = next.account.value().ok_or_else(refused)?;
        let ticket = result.receipt.ticket.value().ok_or_else(refused)?;
        let response = result.result.value().ok_or_else(refused)?;
        let saved_params: Value =
            serde_json::from_str(saved.params.get()).map_err(|_| refused())?;
        let next_params: Value = serde_json::from_str(next.params.get()).map_err(|_| refused())?;
        let valid_scope =
            |scope: &studio_runtime_schema::provider_call_contract::ConnectionScope| {
                scope.account == account.as_str()
                    && !scope.connection.is_empty()
                    && scope.generation > 0
            };
        let authorized_unsubscribe = next.method != TransferMethod::Unsubscribe
            || (member.source_account_key == *account
                && next.operation == format!("transfer:{}:{agent}:unsubscribe", self.operation.id)
                && valid_source_preflight_params(
                    "thread/unsubscribe",
                    member
                        .source_thread_id
                        .value()
                        .map(String::as_str)
                        .unwrap_or_default(),
                    &next_params,
                ));
        if !self.pending()
            || member.phase != Phase::Reading
            || !authorized_unsubscribe
            || saved.operation != next.operation
            || saved.account != next.account
            || saved.method != next.method
            || saved_params != next_params
            || !valid_scope(&next.scope)
            || !valid_scope(&saved.scope)
            || result.receipt.operation != next.operation
            || result.receipt.certainty != ReceiptCertainty::Written
            || result.receipt.scope != saved.scope
            || ticket.generation != saved.scope.generation
            || ticket.method != saved.method
            || !matches!(response.kind(), ResponseKind::Reply)
        {
            return Err(refused());
        }
        Ok(Some(result.clone()))
    }

    /// Journal a provider read independently from the final thread mutation.
    /// An existing Written receipt is replayed, NotSent may retry the same
    /// operation, and an intent without a receipt is held as uncertain.
    pub fn typed_provider_read_intent(
        &self,
        agent: &str,
        intent: &studio_runtime_store::transfer_record::ProviderIntent,
    ) -> Result<Decision, TransferError> {
        use studio_runtime_schema::queue_command::Field;
        use studio_runtime_store::transfer_record::{ReceiptCertainty, TransferMethod};
        let member = self
            .operation
            .members
            .get(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        let account = intent.account.value().ok_or(TransferError::Refused(
            "The transfer is not ready for a provider read",
        ))?;
        let params: Value = serde_json::from_str(intent.params.get())
            .map_err(|_| invalid("provider payload", "/"))?;
        let method = intent.method.as_str();
        let source_guard = matches!(
            intent.method,
            TransferMethod::QueueList
                | TransferMethod::BackgroundList
                | TransferMethod::Unsubscribe
        ) && member.source_account_key == *account
            && member.source_thread_id.value().map(String::as_str)
                == params.get("threadId").and_then(Value::as_str)
            && valid_source_preflight_params(
                method,
                params
                    .get("threadId")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                &params,
            )
            && intent
                .operation
                .starts_with(&format!("transfer:{}:{agent}:", self.operation.id));
        if !self.pending()
            || member.phase != Phase::Reading
            || intent.operation.is_empty()
            || (!matches!(
                intent.method,
                TransferMethod::ModelList | TransferMethod::ThreadRead | TransferMethod::TurnsList
            ) && !source_guard)
            || !params.is_object()
        {
            return Err(TransferError::Refused(
                "The transfer is not ready for a provider read",
            ));
        }
        if let Some(saved) = member
            .provider_read_intents
            .value()
            .and_then(|intents| intents.get(&intent.operation))
        {
            let saved_params: Value = serde_json::from_str(saved.params.get())
                .map_err(|_| invalid("provider payload", "/"))?;
            if saved.operation != intent.operation
                || saved.account != intent.account
                || saved.scope != intent.scope
                || saved.method != intent.method
                || saved_params != params
            {
                return Err(TransferError::Refused("The provider read intent changed"));
            }
            match member
                .provider_read_receipts
                .value()
                .and_then(|receipts| receipts.get(&intent.operation))
                .map(|receipt| &receipt.receipt.certainty)
            {
                Some(ReceiptCertainty::Written | ReceiptCertainty::NotSent) => {
                    return Ok(self.decision(self.operation.clone(), self.agents.clone(), vec![]));
                }
                Some(ReceiptCertainty::Unknown) => {
                    return Err(TransferError::Refused(
                        "The provider read outcome is unknown; it was not repeated",
                    ));
                }
                // No receipt: the retained ledger keys the call, so asking again
                // replays its recorded answer or holds it; it never sends twice.
                None => {}
            }
        }
        let mut op = self.operation.clone();
        let member = op
            .members
            .get_mut(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        if member.provider_read_intents.value().is_none() {
            member.provider_read_intents = Field::Value(BTreeMap::new());
        }
        let Field::Value(intents) = &mut member.provider_read_intents else {
            unreachable!()
        };
        intents.insert(intent.operation.clone(), intent.clone());
        Ok(self.decision(op, self.agents.clone(), vec![]))
    }

    pub fn provider_read_receipt(
        &self,
        agent: &str,
        operation: &str,
        receipt: &SessionReceipt,
    ) -> Result<Decision, TransferError> {
        use studio_runtime_schema::queue_command::Field;
        use studio_runtime_store::transfer_record::{ProviderReadReceipt, ProviderReceipt};
        let member = self
            .operation
            .members
            .get(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        let intent = member
            .provider_read_intents
            .value()
            .and_then(|intents| intents.get(operation))
            .ok_or(TransferError::Refused(
                "Provider read receipt has no durable intent",
            ))?;
        let certainty = receipt_certainty(receipt.certainty());
        let ticket = provider_ticket(receipt)?;
        let (result, _) = read_result(receipt.response())?;
        let record = ProviderReadReceipt {
            receipt: ProviderReceipt {
                operation: receipt.operation.clone(),
                scope: receipt.scope.clone(),
                certainty,
                ticket,
                not_sent_reason: receipt
                    .not_sent_reason()
                    .map_or(Field::Null, |reason| Field::Value(format!("{reason:?}"))),
                result: Field::Missing,
            },
            result,
        };
        if let Some(existing) = member
            .provider_read_receipts
            .value()
            .and_then(|receipts| receipts.get(operation))
            && read_receipts_equal(existing, &record)
        {
            return Ok(self.decision(self.operation.clone(), self.agents.clone(), vec![]));
        }
        if member.phase != Phase::Reading
            || receipt.operation != operation
            || receipt.scope != intent.scope
        {
            return Err(TransferError::Refused(
                "Provider read receipt does not match its intent",
            ));
        }
        let mut op = self.operation.clone();
        let member = op
            .members
            .get_mut(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        if member.provider_read_receipts.value().is_none() {
            member.provider_read_receipts = Field::Value(BTreeMap::new());
        }
        let Field::Value(receipts) = &mut member.provider_read_receipts else {
            unreachable!()
        };
        receipts.insert(operation.to_owned(), record);
        if receipt.certainty() == RequestCertainty::Unknown {
            member.phase = Phase::Unknown;
            member.error =
                Field::Value("Provider read outcome is unknown; no request was repeated".into());
        }
        Ok(self.decision(op, self.agents.clone(), vec![]))
    }

    /// Freeze validated destination settings and an optional portable archive
    /// before any native thread mutation is authorized.
    pub fn prepared_member(
        &self,
        agent: &str,
        target_settings: Value,
        portable_history: Option<Value>,
    ) -> Result<Decision, TransferError> {
        use studio_runtime_schema::queue_command::Field;
        if !self.pending() || self.phase(agent)? != Phase::Reading || !target_settings.is_object() {
            return Err(TransferError::Refused(
                "The transfer is not ready for destination settings",
            ));
        }
        let current = self.agent(agent)?;
        let member = self
            .operation
            .members
            .get(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        let settings = settings_snapshot(current)?;
        let pending = known_transfer_object(&current.pending_settings, "pendingSettings")?
            .map(|value| *value);
        let pending_account = known_transfer_value(
            &current.pending_settings_account_key,
            "pendingSettingsAccountKey",
        )?;
        if member.settings.value() != Some(&settings)
            || member.pending_settings.value() != pending.value()
            || member.pending_settings_account_key.value() != pending_account.value()
        {
            return Err(TransferError::Refused(
                "Agent settings changed during transfer",
            ));
        }
        let mut op = self.operation.clone();
        let member = op
            .members
            .get_mut(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        member.target_settings = Field::Value(
            serde::Deserialize::deserialize(target_settings)
                .map_err(|_| invalid("account transfer", "/members/targetSettings"))?,
        );
        if let Some(history) = portable_history {
            member.portable_history = serde::Deserialize::deserialize(history)
                .map_err(|_| invalid("account transfer", "/members/portableHistory"))?;
        }
        Ok(self.decision(op, self.agents.clone(), vec![]))
    }

    /// Store the exact provider receipt before interpreting its result. Unknown
    /// is durable and never authorizes another mutation.
    ///
    /// A receipt cannot be supplied as independent JSON, result, and certainty:
    /// ```compile_fail
    /// # use studio_runtime_services::account_transfer::Snapshot;
    /// # use studio_runtime_services::account_session_runtime::RequestCertainty;
    /// # fn old_bag(snapshot: &Snapshot) {
    /// snapshot.provider_receipt("agent", "operation", serde_json::Value::Null, None, RequestCertainty::Written);
    /// # }
    /// ```
    pub fn provider_receipt(
        &self,
        agent: &str,
        operation: &str,
        dispatch: &TransferDispatch,
    ) -> Result<Decision, TransferError> {
        use studio_runtime_schema::queue_command::Field;
        let certainty = dispatch.certainty();
        let (receipt_operation, receipt_scope) = dispatch.identity();
        let written_result = dispatch.mutation_result();
        let receipt = dispatch.mutation_record(&written_result)?;
        let member = self
            .operation
            .members
            .get(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        let intent = member.provider_intent.value();
        let saved_receipt = member.provider_receipts.value().is_some_and(|receipts| {
            receipts
                .iter()
                .any(|saved| provider_receipts_equal(saved, &receipt))
        });
        if intent.is_some_and(|intent| intent.operation == operation) && saved_receipt {
            return Ok(self.decision(self.operation.clone(), self.agents.clone(), vec![]));
        }
        if !intent
            .is_some_and(|intent| intent.operation == operation && intent.scope == *receipt_scope)
            || !matches!(
                member.phase,
                Phase::Submitted | Phase::LazySubmitted | Phase::Unknown
            )
            || receipt_operation != operation
        {
            return Err(TransferError::Refused(
                "The provider receipt does not match this transfer intent",
            ));
        }
        let mut op = self.operation.clone();
        let current = op
            .members
            .get_mut(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        if current.provider_receipts.value().is_none() {
            current.provider_receipts = Field::Value(Vec::new());
        }
        let Field::Value(receipts) = &mut current.provider_receipts else {
            unreachable!()
        };
        receipts.push(receipt);
        match certainty {
            RequestCertainty::Written
                if written_result
                    .as_ref()
                    .and_then(|value| value.pointer("/thread/id"))
                    .and_then(Value::as_str)
                    .is_some_and(|id| !id.is_empty()) =>
            {
                current.phase = Phase::Ready;
                current.result = Field::Value(
                    serde_json::value::to_raw_value(&written_result)
                        .map_err(|_| invalid("provider payload", "/"))?,
                );
                current.error = Field::Null;
            }
            RequestCertainty::NotSent => {
                current.phase = if current.lazy.value() == Some(&true) {
                    Phase::Lazy
                } else {
                    Phase::Waiting
                };
                current.error = Field::Value("Provider proved the transfer request was not sent; retry with the same operation id".into());
                current.native_method = Field::Missing;
                current.native_params = Field::Missing;
            }
            RequestCertainty::Written | RequestCertainty::Unknown => {
                current.phase = Phase::Unknown;
                current.error = Field::Value(
                    "Native transfer outcome is unknown; no request was repeated".into(),
                );
            }
        }
        Ok(self.decision(op, self.agents.clone(), vec![]))
    }

    /// Adopt a saved Written thread receipt only after checking the source
    /// snapshot and target settings again in the same store transaction.
    pub fn adopt_receipt(&self, agent: &str) -> Result<Decision, TransferError> {
        use studio_runtime_schema::queue_command::Field;
        use studio_runtime_store::transfer_record::ReceiptCertainty;
        let member = self
            .operation
            .members
            .get(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        if member.phase != Phase::Ready {
            return Err(TransferError::Refused(
                "The transfer has no adoptable thread receipt",
            ));
        }
        let result = member
            .result
            .value()
            .ok_or(TransferError::Refused("The transfer receipt is missing"))?;
        let result: Value =
            serde_json::from_str(result.get()).map_err(|_| invalid("provider payload", "/"))?;
        let target_thread = result
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(TransferError::Refused(
                "The transfer receipt has no thread identity",
            ))?;
        let target_settings = member
            .target_settings
            .value()
            .ok_or(TransferError::Requires(SETTINGS_REQUIRED))?;
        let source = member.source.value().ok_or(TransferError::Refused(
            "The transfer source snapshot is missing",
        ))?;
        let current = self.agent(agent)?;
        if Some(current.epoch) != source.epoch.value().copied()
            || current.thread_id.as_deref() != source.thread_id.value().map(String::as_str)
            || current.cwd.as_deref() != source.cwd.value().map(String::as_str)
        {
            return Err(TransferError::Refused(
                "Agent state changed after history export; saved thread was not adopted",
            ));
        }
        let receipt = member
            .provider_receipts
            .value()
            .and_then(|receipts| receipts.last())
            .ok_or(TransferError::Refused(
                "The transfer provider receipt is not Written",
            ))?;
        let intent = member
            .provider_intent
            .value()
            .ok_or(TransferError::Refused(
                "The transfer provider receipt is not the exact Written intent",
            ))?;
        if receipt.certainty != ReceiptCertainty::Written
            || receipt.operation != intent.operation
            || receipt.scope != intent.scope
            || !receipt.ticket.is_some()
        {
            return Err(TransferError::Refused(
                "The transfer provider receipt is not the exact Written intent",
            ));
        }
        let mut agents = self.agents.clone();
        let a = agents
            .get_mut(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        a.thread_id = Some(target_thread.to_owned());
        a.turn_id = None;
        a.execution_settings_account_key =
            Field::Value(self.operation.target_account_key.as_str().to_owned());
        let settings = &target_settings.settings;
        if let Some(account_key) = &settings.account_key {
            a.account_key = account_key.clone();
        }
        restore_field(
            &settings
                .updated_at
                .clone()
                .map(studio_runtime_store::StoredTransferValue::Known),
            &mut a.updated_at,
        );
        restore_optional(&settings.model, &mut a.model, false);
        restore_optional(&settings.fast_mode, &mut a.fast_mode, false);
        restore_optional(&settings.daybreak_enabled, &mut a.daybreak_enabled, false);
        restore_optional(&settings.permission_mode, &mut a.permission_mode, false);
        restore_field(&settings.provider, &mut a.provider);
        restore_field(&settings.effort, &mut a.effort);
        restore_field(&settings.native_effort, &mut a.native_effort);
        restore_field(&settings.role, &mut a.role);
        restore_field(&settings.profile_instructions, &mut a.profile_instructions);
        restore_field(
            &settings
                .cyber_access_program
                .clone()
                .map(studio_runtime_store::StoredTransferValue::Known),
            &mut a.cyber_access_program,
        );
        restore_field(
            &settings
                .worker_defaults
                .clone()
                .map(studio_runtime_store::StoredWorkerDefaults::Known),
            &mut a.worker_defaults,
        );
        restore_field(
            &settings
                .claude_options
                .clone()
                .map(studio_runtime_store::StoredTransferObject::Known),
            &mut a.claude_options,
        );
        restore_field(&settings.portable_history, &mut a.portable_history);
        restore_field(
            &settings
                .yolo_mode
                .clone()
                .map(studio_runtime_store::StoredTransferValue::Known),
            &mut a.yolo_mode,
        );
        restore_field(
            &target_settings
                .pending_settings_account_key
                .clone()
                .map(studio_runtime_store::StoredTransferValue::Known),
            &mut a.pending_settings_account_key,
        );
        restore_field(
            &target_settings
                .pending_settings
                .clone()
                .map(|value| studio_runtime_store::StoredTransferObject::Known(Box::new(value))),
            &mut a.pending_settings,
        );
        if !member.portable_history.is_missing() {
            a.portable_history = member.portable_history.clone();
        }
        a.lazy_account_transfer = Field::Missing;
        a.account_transfer_id = Field::Missing;
        a.error = Field::Missing;
        a.clear_native_failure_hold();
        a.prepare_attempt = Field::Missing;
        use studio_runtime_store::StoredTransferObject;
        let mut history = studio_runtime_store::transfer_record::TransferAccountHistoryEntry::new(
            member.source_account_key.clone(),
        );
        history.transfer_id = Field::Value(self.operation.id.clone());
        history.thread_id = member
            .source_thread_id
            .value()
            .cloned()
            .map_or(Field::Null, Field::Value);
        history.provider = member
            .provider
            .value()
            .copied()
            .map_or(Field::Null, Field::Value);
        history.target_account_key = Field::Value(self.operation.target_account_key.clone());
        history.target_provider = self
            .operation
            .target_provider
            .value()
            .copied()
            .map_or(Field::Null, Field::Value);
        history.target_thread_id = Field::Value(target_thread.to_owned());
        history.result = Field::Value(result);
        append_history(a, StoredTransferObject::Known(history));
        let mut op = self.operation.clone();
        let member = op
            .members
            .get_mut(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        member.phase = Phase::Completed;
        member.error = Field::Null;
        if op
            .members
            .0
            .iter()
            .all(|(_, member)| member.phase.terminal())
        {
            op.status = TransferStatus::Completed;
        }
        Ok(self.decision(op, agents, vec![]))
    }

    /// Record a transport ticket before the caller sends. A returned saved result
    /// or an uncertain phase never authorizes dispatch, even after restart.
    pub fn submit(
        &self,
        ticket: &Ticket,
        method: studio_runtime_store::transfer_record::TransferMethod,
        params: &serde_json::value::RawValue,
    ) -> Result<Decision, TransferError> {
        use studio_runtime_schema::queue_command::Field;
        use studio_runtime_store::transfer_record::TransferMethod;
        let id = &ticket.agent;
        self.check_ticket_agent(ticket)?;
        let member = self
            .operation
            .members
            .get(id)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        if !self.pending()
            || member.phase != Phase::Reading
            || numeric_field_is_nonzero(&member.submitted_at)
            || payload_field_is_truthy(&member.result)
            || !member.transport_ticket.is_missing()
        {
            return Err(TransferError::Refused(
                "The transfer is not ready for a new native request",
            ));
        }
        let params_value: Value =
            serde_json::from_str(params.get()).map_err(|_| invalid("provider payload", "/"))?;
        if !matches!(method, TransferMethod::Start | TransferMethod::Fork)
            || !params_value.is_object()
        {
            return Err(TransferError::Refused("Invalid native transfer request"));
        }
        let agent = &self.agents[id];
        validate_source_for_submission(member, agent)?;
        let mut op = self.operation.clone();
        let member = op
            .members
            .get_mut(id)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        member.phase = if member.lazy.value() == Some(&true) {
            Phase::LazySubmitted
        } else {
            Phase::Submitted
        };
        member.transport_ticket = Field::Value(ticket.clone());
        member.native_method = Field::Value(method);
        member.native_params = Field::Value(params.to_owned());
        member.target_connection = Field::Value(ticket.connection.clone());
        Ok(self.decision(op, self.agents.clone(), vec![]))
    }

    /// Release a failed preparation without changing a submitted request or a
    /// newer source snapshot. Restart also recovers a crash before this write.
    pub fn typed_preparation_failed(
        &self,
        agent: &str,
        source: &studio_runtime_schema::queue_command::Field<
            studio_runtime_store::transfer_record::TransferSource,
        >,
    ) -> Result<Decision, TransferError> {
        use studio_runtime_schema::queue_command::Field;
        let Some(member) = self.operation.members.get(agent) else {
            return Ok(self.decision(self.operation.clone(), self.agents.clone(), vec![]));
        };
        let source_matches = match (&member.source, source) {
            (Field::Null, Field::Missing | Field::Null) => true,
            (Field::Value(saved), Field::Value(source)) => saved == source,
            (Field::Missing, _)
            | (Field::Null, Field::Value(_))
            | (Field::Value(_), Field::Missing | Field::Null) => false,
        };
        if !self.pending() || member.phase != Phase::Reading || !source_matches {
            return Ok(self.decision(self.operation.clone(), self.agents.clone(), vec![]));
        }
        let mut operation = self.operation.clone();
        let member = operation
            .members
            .get_mut(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        member.phase = Phase::Blocked;
        member.error = Field::Value("Transfer preparation failed before native submission".into());
        Ok(self.decision(operation, self.agents.clone(), vec![]))
    }

    /// Receipts can arrive after restart or cancellation. Preserve the success
    /// without adopting it; adoption requires fresh source and archive checks.
    pub fn typed_receive(
        &self,
        ticket: &Ticket,
        result: Option<studio_runtime_store::transfer_record::ProviderPayload>,
    ) -> Result<Decision, TransferError> {
        let result: Option<Value> = result
            .map(|raw| serde_json::from_str(raw.get()))
            .transpose()
            .map_err(|_| invalid("provider payload", "/"))?;
        use studio_runtime_schema::queue_command::Field;
        let member = self
            .operation
            .members
            .get(&ticket.agent)
            .ok_or(TransferError::Refused(
                "The transfer receipt belongs to another native request",
            ))?;
        if member.transport_ticket.value() != Some(ticket) {
            return Err(TransferError::Refused(
                "The transfer receipt belongs to another native request",
            ));
        }
        self.check_ticket_agent(ticket)?;
        if let Some(saved) = member.result.value() {
            let saved: Value = serde_json::from_str(saved.get())
                .map_err(|_| invalid("account transfer", "/members/result"))?;
            if result.as_ref() == Some(&saved) {
                return Ok(self.decision(self.operation.clone(), self.agents.clone(), vec![]));
            }
            return Err(TransferError::Refused(
                "The transfer receipt conflicts with its saved result",
            ));
        }
        if !matches!(
            self.phase(&ticket.agent)?,
            Phase::Submitted | Phase::LazySubmitted | Phase::Unknown
        ) {
            return Err(TransferError::Refused(
                "The transfer has no pending native receipt",
            ));
        }
        let mut op = self.operation.clone();
        let member = op
            .members
            .get_mut(&ticket.agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        match result {
            Some(result)
                if result
                    .pointer("/thread/id")
                    .and_then(Value::as_str)
                    .is_some_and(|v| !v.is_empty()) =>
            {
                member.phase = Phase::Ready;
                member.result = Field::Value(
                    serde_json::value::to_raw_value(&result)
                        .map_err(|_| invalid("provider payload", "/"))?,
                );
                member.error = Field::Null;
            }
            _ => {
                member.phase = Phase::Unknown;
                member.error = Field::Value(
                    "Native transfer returned no thread identity; outcome unknown".into(),
                );
            }
        }
        Ok(self.decision(op, self.agents.clone(), vec![]))
    }
    fn check_ticket_agent(&self, ticket: &Ticket) -> Result<(), TransferError> {
        let a = self
            .agents
            .get(&ticket.agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        let member = self
            .operation
            .members
            .get(&ticket.agent)
            .ok_or(TransferError::Refused(
                "The transfer receipt identity is stale",
            ))?;
        if self.operation.id != ticket.operation
            || self.operation.target_account_key.as_str() != ticket.account.as_str()
            || a.epoch != ticket.epoch
            || a.is_deleted()
            || a.account_key.as_str()
                != if member.lazy.value() == Some(&true) {
                    self.operation.target_account_key.as_str()
                } else {
                    member.source_account_key.as_str()
                }
            || ticket.request.is_empty()
            || ticket.connection.is_empty()
            || (a.account_transfer_id.value().map(|id| id.as_str())
                != Some(ticket.operation.as_str())
                && self.pending())
        {
            return Err(TransferError::Refused(
                "The transfer receipt identity is stale",
            ));
        }
        Ok(())
    }
}

/// Why a `waiting` or `lazy` member has not moved yet: its move runs at the
/// wake of its next turn, before that turn's input is claimed.
pub const WAITING_FOR_NEXT_TURN: &str = "Moves when its next turn starts";

fn append_history(
    agent: &mut studio_runtime_store::AgentRecord,
    saved: studio_runtime_store::StoredTransferObject<
        studio_runtime_store::transfer_record::TransferAccountHistoryEntry,
    >,
) {
    use studio_runtime_schema::queue_command::Field;
    use studio_runtime_store::{StoredTransferObject, StoredTransferValue};
    match &mut agent.account_history {
        Field::Value(StoredTransferValue::Known(rows)) => rows.push(saved),
        Field::Value(StoredTransferValue::Unreadable(value)) => {
            // Retain the former whole-field value as an unreadable history entry.
            agent.account_history = Field::Value(StoredTransferValue::Known(vec![
                StoredTransferObject::Unreadable(value.clone()),
                saved,
            ]));
        }
        Field::Missing | Field::Null => {
            agent.account_history = Field::Value(StoredTransferValue::Known(vec![saved]))
        }
    }
}

fn restore_optional<T: Clone>(
    saved: &studio_runtime_schema::queue_command::Field<T>,
    current: &mut Option<T>,
    exact: bool,
) {
    use studio_runtime_schema::queue_command::Field;
    match saved {
        Field::Missing if !exact => {}
        Field::Missing | Field::Null => *current = None,
        Field::Value(value) => *current = Some(value.clone()),
    }
}

fn restore_field<T: Clone>(
    saved: &studio_runtime_schema::queue_command::Field<T>,
    current: &mut studio_runtime_schema::queue_command::Field<T>,
) {
    if !saved.is_missing() {
        *current = saved.clone();
    }
}

fn restore_source_field<T: Clone>(
    saved: &studio_runtime_schema::queue_command::Field<T>,
    current: &mut studio_runtime_schema::queue_command::Field<T>,
    exact: bool,
) {
    use studio_runtime_schema::queue_command::Field;
    match saved {
        Field::Missing if !exact => {}
        Field::Missing | Field::Null if !exact => *current = Field::Missing,
        Field::Missing | Field::Null | Field::Value(_) => *current = saved.clone(),
    }
}

fn known_transfer_value<T: Clone>(
    field: &studio_runtime_schema::queue_command::Field<
        studio_runtime_store::StoredTransferValue<T>,
    >,
    key: &str,
) -> Result<studio_runtime_schema::queue_command::Field<T>, TransferError> {
    use studio_runtime_store::StoredTransferValue;
    field.clone().try_map(|value| match value {
        StoredTransferValue::Known(value) => Ok(value),
        StoredTransferValue::Unreadable(_) => Err(invalid("transfer agent", format!("/{key}"))),
    })
}

fn known_transfer_object<T: Clone>(
    field: &studio_runtime_schema::queue_command::Field<
        studio_runtime_store::StoredTransferObject<T>,
    >,
    key: &str,
) -> Result<studio_runtime_schema::queue_command::Field<T>, TransferError> {
    use studio_runtime_store::StoredTransferObject;
    field.clone().try_map(|value| match value {
        StoredTransferObject::Known(value) => Ok(value),
        StoredTransferObject::Unreadable(_) => Err(invalid("transfer agent", format!("/{key}"))),
    })
}

fn worker_defaults(
    agent: &studio_runtime_store::AgentRecord,
) -> Result<
    studio_runtime_schema::queue_command::Field<
        studio_runtime_store::transfer_record::TransferWorkerDefaults,
    >,
    TransferError,
> {
    use studio_runtime_store::StoredWorkerDefaults;
    agent.worker_defaults.clone().try_map(|value| match value {
        StoredWorkerDefaults::Known(value) => Ok(value),
        StoredWorkerDefaults::Unreadable(_) => Err(invalid("transfer agent", "/workerDefaults")),
    })
}

/// The transfer member record for one agent, as a request enrolls it, and the
/// agent's hold. A member on another provider stays on its source account. A
/// member busy with a turn waits for its next turn. An idle member gets the lazy
/// hold: its account changes now, and its history moves at its next turn start.
pub fn typed_enroll(
    agent: &mut studio_runtime_store::AgentRecord,
    operation_id: &studio_runtime_store::transfer_record::TransferId,
    target: &studio_runtime_schema::account_key::AccountKey,
    target_provider: studio_runtime_schema::ProviderKind,
    lead: bool,
) -> Result<studio_runtime_store::transfer_record::TransferMember, TransferError> {
    use studio_runtime_schema::queue_command::Field;
    use studio_runtime_store::AgentStatus;
    use studio_runtime_store::transfer_record::{
        LazySourceState, LazyTransfer, LazyTransferState, TransferMember,
    };
    fn or_null<T>(field: Field<T>) -> Field<T> {
        if field.is_missing() {
            Field::Null
        } else {
            field
        }
    }
    let source: studio_runtime_schema::account_key::AccountKey = agent
        .account_key
        .clone()
        .try_into()
        .map_err(|_| TransferError::Refused("Invalid source account"))?;
    let provider = agent
        .provider
        .value()
        .copied()
        .ok_or(TransferError::Refused("Unknown agent provider"))?;
    let source_thread = agent.thread_id.clone().map_or(Field::Null, Field::Value);
    let mut member = TransferMember::new(Phase::Waiting, source.clone());
    member.source_thread_id = source_thread.clone();
    member.provider = Field::Value(provider);
    member.name = agent.name.clone().map_or(Field::Null, Field::Value);
    if !lead && provider != target_provider {
        member.phase = Phase::Left;
        member.reason = Field::Value(format!(
            "Uses {}; the destination account uses {}",
            provider.key(),
            target_provider.key()
        ));
        return Ok(member);
    }
    if matches!(
        agent.environment.value(),
        Some(studio_runtime_store::StoredTransferValue::Known(
            studio_runtime_schema::project_ingress::WorkerEnvironment::Linux
        ))
    ) && source != *target
    {
        return Err(TransferError::Refused(
            "Archive VM workers before changing the team account",
        ));
    }
    let done = source == *target;
    let active = agent.in_flight == Some(true)
        || matches!(
            agent.status,
            AgentStatus::Running | AgentStatus::Starting | AgentStatus::Approval
        );
    let lazy = !done && !active;
    let pending =
        known_transfer_object(&agent.pending_settings, "pendingSettings")?.map(|value| *value);
    let pending_account = known_transfer_value(
        &agent.pending_settings_account_key,
        "pendingSettingsAccountKey",
    )?;
    let pending_mismatch =
        pending.is_some() && pending_account.value().unwrap_or(&source) != &source;
    let blocked = lazy && pending_mismatch;
    member.phase = if done {
        Phase::Completed
    } else if blocked {
        Phase::Blocked
    } else if active {
        Phase::Waiting
    } else {
        Phase::Lazy
    };
    member.lazy = Field::Value(lazy);
    member.pending_settings = or_null(pending.clone());
    member.source_pending_settings = or_null(pending.clone());
    member.source_claude_options = or_null(known_transfer_object(
        &agent.claude_options,
        "claudeOptions",
    )?);
    member.error = if blocked {
        Field::Value(
            "Queued settings belong to another account. Save them again before transfer".into(),
        )
    } else {
        Field::Null
    };
    if matches!(member.phase, Phase::Waiting | Phase::Lazy) {
        member.waiting = Field::Value(WAITING_FOR_NEXT_TURN.into());
    }
    if done || blocked {
        return Ok(member);
    }
    agent.account_transfer_id = Field::Value(operation_id.clone());
    if !lazy {
        return Ok(member);
    }
    let source_state = LazySourceState {
        account_key: Field::Value(source.clone()),
        provider: Field::Value(provider),
        model: agent.model.clone().map_or(Field::Missing, Field::Value),
        effort: agent.effort.clone(),
        native_effort: agent.native_effort.clone(),
        fast_mode: agent.fast_mode.map_or(Field::Missing, Field::Value),
        daybreak_enabled: agent.daybreak_enabled.map_or(Field::Missing, Field::Value),
        cyber_access_program: known_transfer_value(
            &agent.cyber_access_program,
            "cyberAccessProgram",
        )?,
        worker_defaults: worker_defaults(agent)?,
        pending_settings: pending,
        pending_settings_account_key: pending_account,
        claude_options: known_transfer_object(&agent.claude_options, "claudeOptions")?,
        execution_settings_account_key: agent.execution_settings_account_key.clone().try_map(
            |value| {
                value
                    .try_into()
                    .map_err(|_| invalid("transfer agent", "/executionSettingsAccountKey"))
            },
        )?,
        status: Field::Value(agent.status.clone()),
        error: agent.error.clone().try_map(|value| match value {
            studio_runtime_schema::provider_errors::AgentError::Message(value) => Ok(value),
            studio_runtime_schema::provider_errors::AgentError::Provider(_) => {
                Err(invalid("transfer agent", "/error"))
            }
        })?,
        native_failure_hold: agent
            .native_failure_hold
            .clone()
            .try_map(|value| match value {
                studio_runtime_store::NativeFailureHold::Boolean(value) => Ok(value),
                studio_runtime_store::NativeFailureHold::Unreadable(_) => {
                    Err(invalid("transfer agent", "/nativeFailureHold"))
                }
            })?,
    };
    agent.lazy_account_transfer =
        Field::Value(LazyTransferState::Pending(Box::new(LazyTransfer {
            id: operation_id.clone(),
            source_account_key: source.clone(),
            source_thread_id: source_thread,
            source_state: Field::Value(source_state),
            source_state_exact: Field::Value(true),
        })));
    agent.account_key = target.as_str().to_owned();
    if provider != target_provider {
        agent.provider = Field::Value(target_provider);
        agent.model = Some(crate::provider_adapter::default_model(target_provider).into());
        agent.effort = Field::Null;
        agent.native_effort = Field::Value("medium".into());
        agent.fast_mode = Some(false);
        agent.daybreak_enabled = Some(false);
        agent.cyber_access_program = Field::Value(
            studio_runtime_store::StoredTransferValue::Known("standard".into()),
        );
        agent.claude_options = Field::Missing;
        agent.pending_settings = Field::Missing;
        agent.pending_settings_account_key = Field::Missing;
    }
    let has_pending = agent.pending_settings.is_some();
    agent.pending_settings_account_key = if has_pending {
        Field::Value(studio_runtime_store::StoredTransferValue::Known(
            target.clone(),
        ))
    } else {
        Field::Null
    };
    if !has_pending {
        agent.execution_settings_account_key = Field::Value(source.as_str().to_owned());
    }
    Ok(member)
}

pub fn before_start(agent: &studio_runtime_store::AgentRecord) -> Result<(), TransferError> {
    use studio_runtime_store::transfer_record::{AgentTransferState, TransferScope};
    let summary_holds = match agent.account_transfer.value() {
        None => false,
        Some(AgentTransferState::Unreadable(_)) => true,
        Some(AgentTransferState::Summary(summary)) => {
            !matches!(
                summary.status,
                TransferStatus::Completed | TransferStatus::Cancelled
            ) && !(matches!(summary.status, TransferStatus::Pending)
                && matches!(summary.scope.value(), Some(TransferScope::Subagents))
                && agent.is_lead == Some(true))
        }
    };
    if summary_holds
        || agent.account_transfer_id.value().is_some()
        || agent.lazy_account_transfer.value().is_some()
        || agent.extra.get("emptyTransferRecovery").is_some_and(|r| {
            !r.is_null() && r.get("phase").and_then(Value::as_str) != Some("completed")
        })
    {
        return Err(TransferError::Requires(RECEIPT_HOLD));
    }
    Ok(())
}
fn numeric_field_is_nonzero(
    field: &studio_runtime_schema::queue_command::Field<serde_json::Number>,
) -> bool {
    field
        .value()
        .and_then(serde_json::Number::as_f64)
        .is_some_and(|value| value != 0.0)
}
fn payload_field_is_truthy(
    field: &studio_runtime_schema::queue_command::Field<
        studio_runtime_store::transfer_record::ProviderPayload,
    >,
) -> bool {
    field
        .value()
        .and_then(|raw| serde_json::from_str(raw.get()).ok())
        .is_some_and(|value| studio_runtime_store::truthy(Some(&value)))
}

fn payload_fields_equal(
    left: &studio_runtime_schema::queue_command::Field<
        studio_runtime_store::transfer_record::ProviderPayload,
    >,
    right: &studio_runtime_schema::queue_command::Field<
        studio_runtime_store::transfer_record::ProviderPayload,
    >,
) -> bool {
    use studio_runtime_schema::queue_command::Field;
    match (left, right) {
        (Field::Missing, Field::Missing) | (Field::Null, Field::Null) => true,
        (Field::Value(left), Field::Value(right)) => {
            let left = serde_json::from_str::<Value>(left.get());
            let right = serde_json::from_str::<Value>(right.get());
            matches!((left, right), (Ok(left), Ok(right)) if left == right)
        }
        (Field::Missing | Field::Null, Field::Value(_) | Field::Missing | Field::Null)
        | (Field::Value(_), Field::Missing | Field::Null) => false,
    }
}

fn provider_receipts_equal(
    a: &studio_runtime_store::transfer_record::ProviderReceipt,
    b: &studio_runtime_store::transfer_record::ProviderReceipt,
) -> bool {
    a.operation == b.operation
        && a.scope == b.scope
        && a.certainty == b.certainty
        && a.ticket == b.ticket
        && a.not_sent_reason == b.not_sent_reason
        && payload_fields_equal(&a.result, &b.result)
}

fn read_receipts_equal(
    left: &studio_runtime_store::transfer_record::ProviderReadReceipt,
    right: &studio_runtime_store::transfer_record::ProviderReadReceipt,
) -> bool {
    use studio_runtime_schema::queue_command::Field;
    provider_receipts_equal(&left.receipt, &right.receipt)
        && match (&left.result, &right.result) {
            (Field::Missing, Field::Missing) | (Field::Null, Field::Null) => true,
            (Field::Value(a), Field::Value(b)) => match (a, b) {
                (
                    studio_runtime_store::transfer_record::ProviderResult::Blob {
                        kind: ak,
                        value_blob: av,
                    },
                    studio_runtime_store::transfer_record::ProviderResult::Blob {
                        kind: bk,
                        value_blob: bv,
                    },
                ) => ak == bk && av == bv,
                (
                    studio_runtime_store::transfer_record::ProviderResult::Inline {
                        kind: ak,
                        value: av,
                    },
                    studio_runtime_store::transfer_record::ProviderResult::Inline {
                        kind: bk,
                        value: bv,
                    },
                ) => {
                    ak == bk
                        && match (av, bv) {
                            (None, None) => true,
                            (Some(a), Some(b)) => {
                                serde_json::from_str::<Value>(a.get()).ok()
                                    == serde_json::from_str::<Value>(b.get()).ok()
                            }
                            _ => false,
                        }
                }
                _ => false,
            },
            (Field::Missing | Field::Null, Field::Value(_) | Field::Missing | Field::Null)
            | (Field::Value(_), Field::Missing | Field::Null) => false,
        }
}
pub(crate) fn remove(value: &mut Value, keys: &[&str]) -> Result<(), TransferError> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid("transfer agent", "/"))?;
    for key in keys {
        object.remove(*key);
    }
    Ok(())
}
const CANCEL_FOLLOWUP: &str = "The account transfer was cancelled after this turn was interrupted. Continue the existing task on this account from saved context. Preserve completed work and check existing receipts before any command with an unknown outcome.";

fn check_cancel(
    operation: &studio_runtime_store::transfer_record::TransferRecord,
    agents: &BTreeMap<String, studio_runtime_store::AgentRecord>,
) -> Result<(), TransferError> {
    use studio_runtime_store::transfer_record::InterruptOutcome;
    for (_, member) in &operation.members.0 {
        let phase = member.phase;
        let submitted = numeric_field_is_nonzero(&member.submitted_at);
        let result = payload_field_is_truthy(&member.result);
        // Only a native thread request moves history. Source and
        // catalog reads do not, so a member without one may return.
        let native_request = matches!(
            phase,
            Phase::Reading | Phase::Submitted | Phase::LazySubmitted | Phase::Ready
        ) || member.provider_intent.is_some()
            || member.transport_ticket.is_some()
            || submitted
            || result;
        if member.lazy.value() == Some(&true)
            && !phase.terminal()
            && phase != Phase::Lazy
            && native_request
            && !(phase == Phase::Blocked
                && member.archive_invalidated.value() == Some(&true)
                && (!submitted || result))
        {
            return Err(TransferError::Refused(
                "A native history move has started. Resolve its receipt before cancelling.",
            ));
        }
    }
    if operation.members.0.iter().any(|(_, member)| {
        numeric_field_is_nonzero(&member.interrupt_submitted_at)
            && member.interrupt_outcome.value() != Some(&InterruptOutcome::Acknowledged)
    }) {
        return Err(TransferError::Refused(
            "Retry the unresolved turn interruption before cancelling this transfer",
        ));
    }
    if operation.members.0.iter().any(|(id, member)| {
        member.interrupt_outcome.value() == Some(&InterruptOutcome::Acknowledged)
            && agents[id].in_flight == Some(true)
    }) {
        return Err(TransferError::Refused(
            "Wait for the active turn interruption to finish before cancelling this transfer",
        ));
    }
    Ok(())
}

impl Decision {
    pub fn admit_provider_read(
        &mut self,
        agent: &str,
        operation: &str,
        admission: studio_runtime_store::transfer_record::ProviderAdmission,
    ) -> Result<(), TransferError> {
        use studio_runtime_schema::queue_command::Field;
        let member = self
            .operation
            .members
            .get_mut(agent)
            .ok_or(TransferError::Refused("Unknown transfer member"))?;
        if member.provider_read_admissions.value().is_none() {
            member.provider_read_admissions = Field::Value(BTreeMap::new());
        }
        let Field::Value(admissions) = &mut member.provider_read_admissions else {
            unreachable!()
        };
        if admissions.contains_key(operation) {
            return Err(TransferError::Refused(
                "The transfer provider admission already exists",
            ));
        }
        admissions.insert(operation.to_owned(), admission);
        self.changed = true;
        Ok(())
    }

    pub fn typed_public_view(
        &self,
    ) -> studio_runtime_store::transfer_record::SavedTransferResponse {
        public_view_record(&self.operation)
    }

    /// Atomically maintained renderer summary; a newer operation owns its lead.
    pub fn stamp(&mut self, now: f64, previous_created: Option<f64>) -> Result<(), TransferError> {
        use studio_runtime_schema::queue_command::Field;
        if !self.changed {
            return Ok(());
        }
        let now = serde_json::Number::from_f64(now)
            .ok_or(TransferError::Refused("Invalid transfer timestamp"))?;
        self.operation.updated = Field::Value(now);
        let operation = &self.operation;
        let lead = self
            .agents
            .get_mut(&operation.lead_id)
            .ok_or_else(|| invalid("transfer agent", "/id"))?;
        use studio_runtime_store::transfer_record::AgentTransferState;
        let pointer = lead.account_transfer_id.value().map(|id| id.as_str());
        if pointer.is_some_and(|value| value != operation.id) {
            return Ok(());
        }
        if let Some(AgentTransferState::Summary(current)) = lead.account_transfer.value()
            && current.id.as_str() != operation.id
            && pointer != Some(operation.id.as_str())
            && (!matches!(
                current.status,
                TransferStatus::Completed | TransferStatus::Cancelled
            ) || previous_created
                .is_some_and(|old| operation.created.as_f64().unwrap_or(0.0) <= old))
        {
            return Ok(());
        }
        lead.account_transfer = Field::Value(AgentTransferState::Summary(Box::new(
            summary_record(operation)?,
        )));
        Ok(())
    }
    pub fn encoded(&self) -> Result<String, TransferError> {
        let value = serde::Serialize::serialize(&(&self.operation), serde_json::value::Serializer)
            .map_err(|_| invalid("account transfer", "/"))?;
        validate("PersistedAccountTransferRecord", "account transfer", &value)?;
        let encoded =
            serde_json::to_string(&self.operation).map_err(|_| invalid("account transfer", "/"))?;
        if encoded.len() > MAX_RECORD_BYTES {
            return Err(invalid("account transfer", "/resourceLimit"));
        }
        Ok(encoded)
    }
}
fn public_view_record(
    op: &studio_runtime_store::transfer_record::TransferRecord,
) -> studio_runtime_store::transfer_record::SavedTransferResponse {
    use studio_runtime_schema::queue_command::Field;
    use studio_runtime_store::transfer_record::{
        SavedTransferMember, SavedTransferResponse, TransferNotice, TransferScope,
    };
    fn present<T: Clone>(field: &Field<T>) -> Field<T> {
        match field {
            Field::Value(value) => Field::Value(value.clone()),
            Field::Missing | Field::Null => Field::Missing,
        }
    }
    let mut completed = 0;
    let mut moved = 0;
    let mut pending = 0;
    let mut moving = 0;
    let mut waiting = 0;
    let mut attention = false;
    let mut retry = false;
    let mut interrupted = vec![];
    let mut left = vec![];
    let mut blocked = vec![];
    let mut members = BTreeMap::new();
    for (id, member) in &op.members.0 {
        let phase = member.phase;
        let lazy = member.lazy.value() == Some(&true);
        completed += usize::from(phase.terminal());
        moved += usize::from(lazy || phase == Phase::Completed);
        pending += usize::from(lazy && phase != Phase::Completed);
        moving += usize::from(phase.moving());
        waiting += usize::from(
            !phase.terminal()
                && !matches!(
                    phase,
                    Phase::Blocked | Phase::Unknown | Phase::Lazy | Phase::LazySubmitted
                ),
        );
        attention |= matches!(phase, Phase::Blocked | Phase::Unknown);
        retry |= phase == Phase::Blocked && member.archive_invalidated.value() != Some(&true);
        for (include, reason, destination) in [
            (
                member
                    .interrupt_reason
                    .value()
                    .is_some_and(|reason| !reason.is_empty()),
                &member.interrupt_reason,
                &mut interrupted,
            ),
            (phase == Phase::Left, &member.reason, &mut left),
            (
                matches!(phase, Phase::Blocked | Phase::Unknown),
                &member.error,
                &mut blocked,
            ),
        ] {
            if include {
                destination.push(TransferNotice {
                    id: id.clone(),
                    name: present(&member.name),
                    provider: present(&member.provider),
                    reason: present(reason),
                });
            }
        }
        members.insert(
            id.clone(),
            SavedTransferMember {
                phase,
                source_account_key: Field::Value(member.source_account_key.clone()),
                source_thread_id: member.source_thread_id.clone(),
                target_thread_id: member.target_thread_id.clone(),
                provider: member.provider.clone(),
                name: member.name.clone(),
                reason: member.reason.clone(),
                error: member.error.clone(),
                waiting: member.waiting.clone(),
                lazy: member.lazy.clone(),
                interrupt_reason: member.interrupt_reason.clone(),
                interrupt_outcome: member.interrupt_outcome.clone(),
                continue_after_transfer: member.continue_after_transfer.clone(),
            },
        );
    }
    SavedTransferResponse {
        id: op.id.clone(),
        lead_id: op.lead_id.clone(),
        target_account_key: op.target_account_key.clone(),
        status: op.status,
        created: op.created.clone(),
        scope: op.scope.value().cloned().unwrap_or(TransferScope::Team),
        members,
        total: op.members.0.len(),
        completed,
        moved,
        native_history_pending: pending,
        moving_now: moving,
        can_finish_history: false,
        waiting_count: waiting,
        needs_attention: attention,
        can_retry: retry,
        interrupted,
        left_on_source: left,
        blocked,
        target_provider: op.target_provider.clone(),
        updated: op.updated.clone(),
        finish_history: op.finish_history.clone(),
    }
}
fn null_if_missing<T: Clone>(
    field: &studio_runtime_schema::queue_command::Field<T>,
) -> studio_runtime_schema::queue_command::Field<T> {
    if field.is_missing() {
        studio_runtime_schema::queue_command::Field::Null
    } else {
        field.clone()
    }
}
fn summary_record(
    op: &studio_runtime_store::transfer_record::TransferRecord,
) -> Result<studio_runtime_store::transfer_record::AgentTransferSummary, TransferError> {
    use studio_runtime_schema::queue_command::Field;
    use studio_runtime_store::transfer_record::{AgentTransferSummary, TransferNotice};
    fn notices(values: Vec<TransferNotice>, provider: bool) -> Vec<TransferNotice> {
        values
            .into_iter()
            .map(|notice| TransferNotice {
                id: notice.id,
                name: null_if_missing(&notice.name),
                provider: if provider {
                    null_if_missing(&notice.provider)
                } else {
                    Field::Missing
                },
                reason: null_if_missing(&notice.reason),
            })
            .collect()
    }
    let view = public_view_record(op);
    let count = |value: usize| -> Result<studio_runtime_schema::AgentCounter, TransferError> {
        i64::try_from(value)
            .ok()
            .and_then(studio_runtime_schema::AgentCounter::from_i64)
            .ok_or_else(|| invalid("account transfer", "/count"))
    };
    let updated = match &op.updated {
        Field::Missing | Field::Null => Field::Null,
        Field::Value(value) => Field::Value(
            value
                .as_f64()
                .ok_or_else(|| invalid("account transfer", "/updated"))?,
        ),
    };
    let waiting = op
        .members
        .0
        .iter()
        .find_map(|(_, member)| {
            member
                .error
                .value()
                .filter(|value| !value.is_empty())
                .or_else(|| member.waiting.value().filter(|value| !value.is_empty()))
        })
        .cloned()
        .map_or(Field::Null, Field::Value);
    Ok(AgentTransferSummary {
        id: op
            .id
            .clone()
            .try_into()
            .map_err(|_| invalid("account transfer", "/id"))?,
        target_account_key: op.target_account_key.clone(),
        status: op.status,
        updated,
        scope: null_if_missing(&op.scope),
        finish_history: Field::Value(true),
        total: Field::Value(count(view.total)?),
        completed: Field::Value(count(view.completed)?),
        moved: Field::Value(count(view.moved)?),
        native_history_pending: Field::Value(count(view.native_history_pending)?),
        moving_now: Field::Value(count(view.moving_now)?),
        can_finish_history: Field::Value(view.can_finish_history),
        waiting_count: Field::Value(count(view.waiting_count)?),
        needs_attention: Field::Value(view.needs_attention),
        can_retry: Field::Value(view.can_retry),
        can_cancel: Field::Missing,
        interrupted: Field::Value(notices(view.interrupted, false)),
        left_on_source: Field::Value(notices(view.left_on_source, true)),
        blocked: Field::Value(notices(view.blocked, false)),
        waiting,
    })
}

fn settings_snapshot(
    agent: &studio_runtime_store::AgentRecord,
) -> Result<studio_runtime_store::transfer_record::TransferSettings, TransferError> {
    use studio_runtime_schema::queue_command::Field;
    use studio_runtime_store::transfer_record::TransferSettings;
    Ok(TransferSettings {
        account_key: None,
        updated_at: Field::Missing,
        provider: null_if_missing(&agent.provider),
        model: agent.model.clone().map_or(Field::Null, Field::Value),
        permission_mode: agent.permission_mode.map_or(Field::Null, Field::Value),
        effort: null_if_missing(&agent.effort),
        native_effort: null_if_missing(&agent.native_effort),
        fast_mode: agent.fast_mode.map_or(Field::Null, Field::Value),
        daybreak_enabled: agent.daybreak_enabled.map_or(Field::Missing, Field::Value),
        cyber_access_program: known_transfer_value(
            &agent.cyber_access_program,
            "cyberAccessProgram",
        )?,
        worker_defaults: null_if_missing(&worker_defaults(agent)?),
        claude_options: null_if_missing(&known_transfer_object(
            &agent.claude_options,
            "claudeOptions",
        )?),
        role: null_if_missing(&agent.role),
        profile_instructions: null_if_missing(&agent.profile_instructions),
        yolo_mode: Field::Missing,
        portable_history: Field::Missing,
    })
}

fn validate_source_for_submission(
    member: &studio_runtime_store::transfer_record::TransferMember,
    agent: &studio_runtime_store::AgentRecord,
) -> Result<(), TransferError> {
    use studio_runtime_schema::queue_command::Field;
    if agent.in_flight == Some(true)
        || matches!(
            agent.status,
            studio_runtime_store::AgentStatus::Running
                | studio_runtime_store::AgentStatus::Starting
                | studio_runtime_store::AgentStatus::Approval
        )
        || agent
            .workspace_operation
            .value()
            .is_some_and(|value| !value.is_empty())
    {
        return Err(TransferError::Refused(
            "Waiting for the current turn or workspace operation",
        ));
    }
    let source = member
        .source
        .value()
        .ok_or_else(|| invalid("account transfer", "/members/source"))?;
    fn matches_field<T: PartialEq>(saved: &Field<T>, current: Option<&T>) -> bool {
        saved.is_missing() || saved.value() == current
    }
    if (source.epoch.is_missing()
        && source.account_key.is_missing()
        && source.thread_id.is_missing()
        && source.cwd.is_missing())
        || !matches_field(&source.epoch, Some(&agent.epoch))
        || (!source.account_key.is_missing()
            && source.account_key.value().map(|key| key.as_str())
                != Some(agent.account_key.as_str()))
        || !matches_field(&source.thread_id, agent.thread_id.as_ref())
        || !matches_field(&source.cwd, agent.cwd.as_ref())
        || member.settings.value() != Some(&settings_snapshot(agent)?)
        || member.pending_settings.value()
            != known_transfer_object(&agent.pending_settings, "pendingSettings")?
                .map(|value| *value)
                .value()
        || member.pending_settings_account_key.value()
            != known_transfer_value(
                &agent.pending_settings_account_key,
                "pendingSettingsAccountKey",
            )?
            .value()
    {
        return Err(TransferError::Refused(
            "Agent state changed during transfer. Its original session is preserved.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod provider_receipt_tests {
    #[test]
    fn environment_is_owned_by_the_typed_agent() {
        for environment in [json!("linux"), json!("future"), json!({"legacy":true})] {
            let value = json!({"compactions":0,"provider":"codex","id":"agent","rootId":"agent",
                "epoch":7,"status":"idle","accountKey":"source","environment":environment});
            let agent = studio_runtime_store::AgentRecord::from_value(value.clone()).unwrap();
            assert!(!agent.extra.contains_key("environment"));
            assert_eq!(agent.value()["environment"], value["environment"]);
        }
    }
    #[test]
    fn optional_source_patch_preserves_legacy_and_exact_semantics() {
        use studio_runtime_schema::queue_command::Field;
        let mut current = Some("current".to_owned());
        super::restore_optional(&Field::Missing, &mut current, false);
        assert_eq!(current.as_deref(), Some("current"));
        super::restore_optional(&Field::Null, &mut current, false);
        assert_eq!(current, None);
        super::restore_optional(&Field::Value("saved".to_owned()), &mut current, false);
        assert_eq!(current.as_deref(), Some("saved"));
        super::restore_optional(&Field::Missing, &mut current, true);
        assert_eq!(current, None);
    }

    #[test]
    fn public_scope_defaults_to_team_when_missing() {
        use studio_runtime_store::transfer_record::TransferScope;
        let base = serde_json::json!({"id":"move","leadId":"agent","targetAccountKey":"target","status":"pending","created":1,"members":{"agent":{"phase":"waiting","sourceAccountKey":"source"}}});
        let agent = serde_json::json!({"id":"agent","rootId":"agent","provider":"codex","accountKey":"source","epoch":0,"status":"idle","compactions":0});
        let snapshot =
            super::Snapshot::read(&base.to_string(), [("agent".into(), agent.to_string())])
                .unwrap();
        assert_eq!(snapshot.typed_public_view().scope, TransferScope::Team);
    }

    #[test]
    fn typed_preparation_failure_distinguishes_missing_and_null_source() {
        use studio_runtime_schema::queue_command::Field;
        let raw = r#"{"id":"move","leadId":"agent","targetAccountKey":"target","status":"pending","created":1,"members":{"agent":{"phase":"reading","sourceAccountKey":"source","source":null}}}"#;
        let agent = serde_json::json!({"id":"agent","rootId":"agent","provider":"codex","accountKey":"source","epoch":0,"status":"idle","compactions":0});
        let snapshot = super::Snapshot::read(raw, [("agent".into(), agent.to_string())]).unwrap();
        assert!(
            snapshot
                .typed_preparation_failed("agent", &Field::Missing)
                .unwrap()
                .changed
        );
        let decision = snapshot
            .typed_preparation_failed("agent", &Field::Null)
            .unwrap();
        assert!(decision.changed);
        assert_eq!(
            decision.operation.members.get("agent").unwrap().phase,
            super::Phase::Blocked
        );
        let absent = raw.replace(",\"source\":null", "");
        let snapshot =
            super::Snapshot::read(&absent, [("agent".into(), agent.to_string())]).unwrap();
        assert!(
            !snapshot
                .typed_preparation_failed("agent", &Field::Null)
                .unwrap()
                .changed
        );
    }

    #[test]
    fn enrollment_reads_typed_pending_settings_and_captures_source() {
        let mut agent: studio_runtime_store::AgentRecord = serde_json::from_str(r#"{"id":"agent","rootId":"agent","provider":"codex","accountKey":"source","epoch":0,"status":"idle","compactions":0,"pendingSettings":{"model":"typed","updatedAt":12.0},"pendingSettingsAccountKey":"source"}"#).unwrap();
        agent.extra.insert(
            "pendingSettings".into(),
            serde_json::json!({"model":"stale"}),
        );
        let id = "move".to_owned().try_into().unwrap();
        let target = "target".to_owned().try_into().unwrap();
        let member = super::typed_enroll(
            &mut agent,
            &id,
            &target,
            studio_runtime_schema::ProviderKind::Codex,
            true,
        )
        .unwrap();
        assert_eq!(
            member
                .pending_settings
                .value()
                .unwrap()
                .model
                .value()
                .map(String::as_str),
            Some("typed")
        );
        let source = agent
            .lazy_account_transfer
            .value()
            .unwrap()
            .pending()
            .unwrap()
            .source_state
            .value()
            .unwrap();
        assert_eq!(
            source.pending_settings.value().unwrap().updated_at.value(),
            Some(&12.0)
        );
        assert_eq!(
            source
                .pending_settings_account_key
                .value()
                .unwrap()
                .as_str(),
            "source"
        );
        assert!(
            matches!(agent.pending_settings_account_key.value(), Some(studio_runtime_store::StoredTransferValue::Known(key)) if key.as_str() == "target")
        );
    }

    #[test]
    fn enrollment_preserves_known_and_legacy_worker_defaults() {
        for defaults in [
            serde_json::json!({"model":"m","effort":null,"fastMode":false}),
            serde_json::json!({"model":"legacy"}),
        ] {
            let mut agent: studio_runtime_store::AgentRecord =
                serde_json::from_str(r#"{"id":"agent","rootId":"agent","provider":"codex","accountKey":"source","epoch":0,"status":"idle","compactions":0}"#).unwrap();
            agent.worker_defaults = serde::Deserialize::deserialize(defaults.clone()).unwrap();
            let id = "move".to_owned().try_into().unwrap();
            let target = "target".to_owned().try_into().unwrap();
            super::typed_enroll(
                &mut agent,
                &id,
                &target,
                studio_runtime_schema::ProviderKind::Codex,
                true,
            )
            .unwrap();
            let lazy = agent
                .lazy_account_transfer
                .value()
                .and_then(|value| value.pending())
                .unwrap();
            let source = lazy.source_state.value().unwrap();
            assert_eq!(
                serde::Serialize::serialize(
                    &(&source.worker_defaults),
                    serde_json::value::Serializer
                )
                .unwrap(),
                defaults
            );
        }
    }

    #[test]
    fn enrollment_reads_typed_worker_defaults_without_projecting_agent() {
        let mut agent: studio_runtime_store::AgentRecord =
            serde_json::from_str(r#"{"id":"agent","rootId":"agent","provider":"codex","accountKey":"source","epoch":0,"status":"idle","compactions":0,"workerDefaults":{"model":"typed","effort":null,"fastMode":false}}"#)
            .unwrap();
        // A stale extension must not replace the typed owner during enrollment.
        agent.extra.insert(
            "workerDefaults".into(),
            serde_json::json!({"model":"stale"}),
        );
        let id = "move".to_owned().try_into().unwrap();
        let target = "target".to_owned().try_into().unwrap();
        super::typed_enroll(
            &mut agent,
            &id,
            &target,
            studio_runtime_schema::ProviderKind::Codex,
            true,
        )
        .unwrap();
        let lazy = agent
            .lazy_account_transfer
            .value()
            .and_then(|value| value.pending())
            .unwrap();
        let source = lazy.source_state.value().unwrap();
        let defaults = source.worker_defaults.value().unwrap();
        assert_eq!(defaults.model.value().map(String::as_str), Some("typed"));
    }

    #[test]
    fn enrollment_provider_change_writes_typed_settings() {
        let mut agent: studio_runtime_store::AgentRecord =
            serde_json::from_str(r#"{"id":"agent","rootId":"agent","provider":"codex","accountKey":"source","epoch":0,"status":"idle","compactions":0,"cyberAccessProgram":"other","claudeOptions":{"configDir":"profile"}}"#)
            .unwrap();
        let id = "move".to_owned().try_into().unwrap();
        let target = "target".to_owned().try_into().unwrap();
        super::typed_enroll(
            &mut agent,
            &id,
            &target,
            studio_runtime_schema::ProviderKind::Claude,
            true,
        )
        .unwrap();
        assert!(agent.claude_options.is_missing());
        assert!(
            matches!(agent.cyber_access_program.value(), Some(studio_runtime_store::StoredTransferValue::Known(value)) if value == "standard")
        );
        assert!(!agent.extra.contains_key("cyberAccessProgram"));
        assert!(!agent.extra.contains_key("claudeOptions"));
        assert!(!agent.extra.contains_key("nativeEffort"));
        assert!(!agent.extra.contains_key("daybreakEnabled"));
    }

    #[test]
    fn typed_boundary_exposes_agent_and_public_transfer() {
        let raw = r#"{"id":"move","leadId":"agent","targetAccountKey":"target","status":"pending","created":1,"members":{"agent":{"phase":"waiting","sourceAccountKey":"source"}}}"#;
        let agent = serde_json::json!({"id":"agent","rootId":"agent","provider":"codex","accountKey":"source","epoch":0,"status":"idle","compactions":0});
        let snapshot = super::Snapshot::read(raw, [("agent".into(), agent.to_string())]).unwrap();
        assert_eq!(snapshot.typed_public_view().total, 1);
        let mut agent = snapshot.agent("agent").unwrap().clone();
        let id = "move".to_owned().try_into().unwrap();
        let target = "target".to_owned().try_into().unwrap();
        let member = super::typed_enroll(
            &mut agent,
            &id,
            &target,
            studio_runtime_schema::ProviderKind::Codex,
            true,
        )
        .unwrap();
        assert_eq!(member.phase, super::Phase::Lazy);
        assert_eq!(agent.account_transfer_id.value().unwrap().as_str(), "move");
        let mut decision = snapshot.decide(super::Command::Restart).unwrap();
        assert_eq!(decision.typed_public_view().total, 1);
        decision.agents.insert("agent".into(), agent);
        assert_eq!(
            decision.agents["agent"]
                .account_transfer_id
                .value()
                .unwrap()
                .as_str(),
            "move"
        );
    }

    #[test]
    fn typed_decision_preserves_member_order() {
        let raw = r#"{"id":"move","leadId":"z","targetAccountKey":"target","status":"pending","created":1,"members":{"z":{"phase":"waiting","sourceAccountKey":"source"},"a":{"phase":"waiting","sourceAccountKey":"source"}}}"#;
        let agent = |id: &str| {
            (id.to_owned(), serde_json::json!({"id":id,"rootId":"z","provider":"codex","accountKey":"source","epoch":0,"status":"idle","compactions":0}).to_string())
        };
        let snapshot = super::Snapshot::read(raw, [agent("z"), agent("a")]).unwrap();
        let decision = snapshot.decide(super::Command::Restart).unwrap();
        let record = &decision.operation;
        assert_eq!(
            record
                .members
                .0
                .iter()
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>(),
            ["z", "a"]
        );
        let encoded = decision.encoded().unwrap();
        assert!(encoded.find("\"z\":{").unwrap() < encoded.find("\"a\":{").unwrap());
    }

    #[test]
    fn spilled_provider_reply_has_stable_bytes_and_digest() {
        let response = super::ProviderResponse::Reply(serde_json::json!("x".repeat(65537)));
        let (result, spill) = super::read_result(Some(&response)).expect("large reply");
        let (digest, bytes) = spill.expect("large reply spills");
        assert_eq!(
            digest,
            "7d695b80524fb81f4595aeb033d2661666a31db1feb18e1e3a8589cd6e7c2fe9"
        );
        assert_eq!(bytes, serde_json::to_vec(&"x".repeat(65537)).unwrap());
        assert_eq!(result.value().unwrap().value_blob().unwrap().sha256, digest);
    }
    use super::*;
    trait TestIntentFixtures {
        fn provider_intent(
            &self,
            agent: &str,
            operation: &str,
            scope: Value,
            method: &str,
            params: Value,
        ) -> Result<Decision, TransferError>;
        fn provider_read_intent(
            &self,
            agent: &str,
            operation: &str,
            account: &str,
            scope: Value,
            method: &str,
            params: Value,
        ) -> Result<Decision, TransferError>;
    }
    impl TestIntentFixtures for Snapshot {
        fn provider_intent(
            &self,
            agent: &str,
            operation: &str,
            scope: Value,
            method: &str,
            params: Value,
        ) -> Result<Decision, TransferError> {
            use studio_runtime_store::transfer_record::TransferMethod;
            let scope = serde::Deserialize::deserialize(scope).map_err(|_| {
                TransferError::Refused("The transfer is not ready for provider submission")
            })?;
            let method = match method {
                "thread/start" => TransferMethod::Start,
                "thread/fork" => TransferMethod::Fork,
                _ => {
                    return Err(TransferError::Refused(
                        "The transfer is not ready for provider submission",
                    ));
                }
            };
            let params = serde_json::value::to_raw_value(&params)
                .map_err(|_| invalid("provider payload", "/"))?;
            self.typed_provider_intent(agent, operation, scope, method, &params)
        }

        fn provider_read_intent(
            &self,
            agent: &str,
            operation: &str,
            account: &str,
            scope: Value,
            method: &str,
            params: Value,
        ) -> Result<Decision, TransferError> {
            use studio_runtime_schema::queue_command::Field;
            use studio_runtime_store::transfer_record::ProviderIntent;
            let refusal =
                || TransferError::Refused("The transfer is not ready for a provider read");
            let intent = ProviderIntent {
                operation: operation.into(),
                account: Field::Value(account.to_owned().try_into().map_err(|_| refusal())?),
                scope: serde::Deserialize::deserialize(scope).map_err(|_| refusal())?,
                method: serde::Deserialize::deserialize(Value::String(method.into()))
                    .map_err(|_| refusal())?,
                params: serde_json::value::to_raw_value(&params)
                    .map_err(|_| invalid("provider payload", "/"))?,
            };
            self.typed_provider_read_intent(agent, &intent)
        }
    }
    use crate::account_session_runtime::SessionOutcome;
    use studio_runtime_providers::{Method, RequestTicket};

    fn test_session(
        raw: Value,
        result: Option<Value>,
        certainty: RequestCertainty,
    ) -> SessionReceipt {
        let mut scope = raw["scope"].clone();
        if scope.get("generation").is_none() {
            scope["generation"] = json!(1);
        }
        let scope: ConnectionScope = serde::Deserialize::deserialize(scope).expect("scope");
        let ticket = RequestTicket {
            generation: raw["ticket"]["generation"]
                .as_u64()
                .unwrap_or(scope.generation),
            id: serde::Deserialize::deserialize(raw["ticket"]["id"].clone()).expect("ticket id"),
            method: Method::from_wire(raw["ticket"]["method"].as_str().unwrap_or("thread/start"))
                .expect("method"),
        };
        let outcome = match certainty {
            RequestCertainty::Written => {
                let result = result.expect("written response");
                let value = if result.get("kind").is_some() {
                    result["value"].clone()
                } else {
                    result
                };
                SessionOutcome::Written {
                    ticket,
                    response: ProviderResponse::Reply(value),
                }
            }
            RequestCertainty::NotSent => SessionOutcome::WorkerNotSent { ticket },
            RequestCertainty::Unknown => SessionOutcome::Unknown { ticket },
        };
        SessionReceipt {
            scope,
            operation: raw["operation"].as_str().expect("operation").into(),
            outcome,
            replayed: false,
        }
    }

    fn test_dispatch(
        raw: Value,
        result: Option<Value>,
        certainty: RequestCertainty,
    ) -> TransferDispatch {
        if raw["ticket"].is_null() {
            let mut scope = raw["scope"].clone();
            if scope.get("generation").is_none() {
                scope["generation"] = json!(1);
            }
            let scope = serde::Deserialize::deserialize(scope).expect("scope");
            let operation = raw["operation"].as_str().expect("operation").into();
            match certainty {
                RequestCertainty::Unknown => TransferDispatch::Unknown { operation, scope },
                RequestCertainty::NotSent => panic!("NotSent requires a retained receipt"),
                RequestCertainty::Written => panic!("written requires a ticket"),
            }
        } else {
            TransferDispatch::Receipt(test_session(raw, result, certainty))
        }
    }

    fn snapshot() -> Snapshot {
        let operation = json!({
            "id":"transfer", "leadId":"agent", "targetAccountKey":"target",
            "status":"pending", "created":1.0, "scope":"team",
            "members":{"agent":{"phase":"lazy", "sourceAccountKey":"source",
                "sourceThreadId":"source-thread", "lazy":true}}
        });
        let agent = json!({"compactions":0,"provider":studio_runtime_schema::ProviderKind::Codex,
            "id":"agent", "rootId":"agent", "epoch":7, "status":"idle",
            "accountKey":"target", "accountTransferId":"transfer",
            "lazyAccountTransfer":{"id":"transfer","sourceAccountKey":"source"}, "model":"model-a"
        });
        Snapshot::read(
            &operation.to_string(),
            [("agent".into(), agent.to_string())],
        )
        .expect("test fixture operation succeeds")
    }

    #[test]
    fn cancellation_succeeds_for_incomplete_exact_source_snapshot() {
        let operation = json!({"id":"transfer","leadId":"agent","targetAccountKey":"target",
            "status":"pending","created":1.0,"scope":"team",
            "members":{"agent":{"phase":"lazy","sourceAccountKey":"source","lazy":true}}});
        let agent = json!({"compactions":0,"provider":"codex","id":"agent","rootId":"agent",
            "epoch":7,"status":"idle","accountKey":"target","accountTransferId":"transfer",
            "lazyAccountTransfer":{"id":"transfer","sourceAccountKey":"source",
                "sourceStateExact":true,"sourceState":{"model":"saved"}}});
        let snapshot = Snapshot::read(
            &operation.to_string(),
            [("agent".into(), agent.to_string())],
        )
        .unwrap();
        let decision = snapshot.decide(Command::Action(Action::Cancel)).unwrap();
        assert_eq!(decision.operation.status, TransferStatus::Cancelled);
        assert_eq!(decision.agents["agent"].model.as_deref(), Some("saved"));
        assert!(
            decision
                .operation
                .members
                .get("agent")
                .unwrap()
                .error
                .is_some()
        );
        let saved = decision.agents["agent"].value();
        assert!(saved.get("lazyAccountTransfer").is_none());
        assert_eq!(saved["accountHistory"][0], agent["lazyAccountTransfer"]);
        assert!(super::before_start(&decision.agents["agent"]).is_ok());
    }

    #[test]
    fn cancellation_preserves_non_array_history_and_incomplete_snapshot() {
        let operation = json!({"id":"transfer","leadId":"agent","targetAccountKey":"target",
            "status":"pending","created":1.0,"scope":"team",
            "members":{"agent":{"phase":"lazy","sourceAccountKey":"source","lazy":true}}});
        let lazy = json!({"id":"transfer","sourceAccountKey":"source",
            "sourceStateExact":true,"sourceState":{"model":"saved"}});
        let agent = json!({"compactions":0,"provider":"codex","id":"agent","rootId":"agent",
            "epoch":7,"status":"idle","accountKey":"target","accountTransferId":"transfer",
            "accountHistory":"legacy", "lazyAccountTransfer":lazy});
        let snapshot = Snapshot::read(
            &operation.to_string(),
            [("agent".into(), agent.to_string())],
        )
        .unwrap();
        let decision = snapshot.decide(Command::Action(Action::Cancel)).unwrap();
        let saved = decision.agents["agent"].value();
        assert_eq!(saved["accountHistory"], json!(["legacy", lazy]));
        assert!(super::before_start(&decision.agents["agent"]).is_ok());
    }

    fn after(decision: Decision) -> Snapshot {
        Snapshot::read(
            &serde_json::to_string(&decision.operation).unwrap(),
            decision
                .agents
                .into_iter()
                .map(|(id, agent)| (id, serde_json::to_string(&agent).unwrap())),
        )
        .expect("test fixture operation succeeds")
    }

    fn intent(snapshot: &Snapshot) -> Decision {
        after(
            snapshot
                .begin_read("agent")
                .expect("test fixture operation succeeds"),
        )
        .provider_intent(
            "agent",
            "transfer:agent:thread-start",
            json!({"account":"target","connection":"connection-1","generation":1}),
            "thread/start",
            json!({"model":"model-a"}),
        )
        .expect("test fixture operation succeeds")
    }

    /// The live record shape of a lazy move refused during its source reads:
    /// written destination catalog and source reads, one source read with an
    /// intent and no receipt, and no native thread request.
    fn refused_during_source_reads(native_intent: bool) -> Snapshot {
        let read = |operation: &str, account: &str, method: &str| {
            json!({"operation":operation,"account":account,"method":method,
                "params":{"threadId":"source-thread"},
                "scope":{"account":account,"connection":format!("{account}-connection"),"generation":1}})
        };
        let written = |operation: &str, account: &str| {
            json!({"certainty":"written","result":{"kind":"reply","value":{"data":[]}},
                "receipt":{"certainty":"written","notSentReason":null,"operation":operation,
                    "scope":{"account":account,"connection":format!("{account}-connection"),"generation":1},
                    "ticket":{"generation":1,"id":"request","method":"thread/read"}}})
        };
        let mut member = json!({
            "phase":"blocked", "lazy":true, "sourceAccountKey":"source",
            "sourceThreadId":"source-thread", "provider":"claude", "nextCheck":0,
            "error":"Transfer preparation failed before native submission",
            "source":{"accountKey":"target","threadId":"source-thread","epoch":7,"cwd":"/work"},
            "providerReadIntents":{
                "model-list":read("model-list","target","model/list"),
                "thread-read":read("thread-read","source","thread/read"),
                "transfer:transfer:agent:queue-before":read("transfer:transfer:agent:queue-before","source","thread/queue/list"),
                "transfer:transfer:agent:background-before":read("transfer:transfer:agent:background-before","source","thread/backgroundTerminals/list")
            },
            "providerReadReceipts":{
                "model-list":written("model-list","target"),
                "thread-read":written("thread-read","source"),
                "transfer:transfer:agent:queue-before":written("transfer:transfer:agent:queue-before","source")
            }
        });
        if native_intent {
            member["providerIntent"] = json!({"operation":"transfer:agent:thread-start",
                "scope":{"account":"target","connection":"target-connection","generation":1},"method":"thread/start","params":{}});
        }
        let operation = json!({
            "id":"transfer", "leadId":"agent", "targetAccountKey":"target",
            "targetProvider":"claude", "status":"pending", "created":1.0, "scope":"team",
            "members":{"agent":member}
        });
        let agent = json!({"compactions":0,
            "id":"agent", "rootId":"agent", "epoch":7, "status":"queued", "provider":"claude",
            "accountKey":"target", "accountTransferId":"transfer", "threadId":"source-thread",
            "executionSettingsAccountKey":"source", "model":"model-a",
            "lazyAccountTransfer":{"id":"transfer","sourceAccountKey":"source",
                "sourceThreadId":"source-thread",
                "sourceState":{"executionSettingsAccountKey":"source","model":"model-a",
                    "provider":"claude","status":"queued","error":null}}
        });
        Snapshot::read(
            &operation.to_string(),
            [("agent".into(), agent.to_string())],
        )
        .expect("test fixture operation succeeds")
    }

    #[test]
    fn blocked_native_intent_does_not_offer_cancel() {
        let snapshot = refused_during_source_reads(true);
        let mut decision = Decision {
            operation: snapshot.operation.clone(),
            agents: snapshot.agents.clone(),
            followups: vec![],
            changed: true,
        };
        decision.stamp(2.0, None).unwrap();
        let summary = decision.agents["agent"].account_transfer.value().unwrap();
        let progress = progress_view(Some(summary), snapshot.can_cancel());
        let encoded =
            serde::Serialize::serialize(&(progress), serde_json::value::Serializer).unwrap();
        assert!(!matches!(
            encoded["actions"].as_str(),
            Some("cancel" | "retry_cancel")
        ));
    }

    #[test]
    fn cancel_returns_a_lazy_member_whose_move_sent_no_thread_request() {
        let cancelled = refused_during_source_reads(false)
            .decide(Command::Action(Action::Cancel))
            .expect("nothing native was sent, so cancel is safe");
        assert_eq!(cancelled.operation.status, TransferStatus::Cancelled);
        let agent = &cancelled.agents["agent"];
        assert_eq!(agent.account_key, "source");
        assert_eq!(
            agent
                .execution_settings_account_key
                .value()
                .map(String::as_str),
            Some("source")
        );
        assert!(agent.lazy_account_transfer.is_missing());
        assert!(agent.account_transfer_id.is_missing());

        assert!(matches!(
            refused_during_source_reads(true).decide(Command::Action(Action::Cancel)),
            Err(TransferError::Refused(
                "A native history move has started. Resolve its receipt before cancelling."
            ))
        ));
    }

    #[test]
    fn provider_intent_precedes_receipt_and_exact_replay_is_idempotent() {
        let submitted = after(intent(&snapshot()));
        let member = submitted.operation.members.get("agent").unwrap();
        assert_eq!(member.phase, Phase::LazySubmitted);
        assert_eq!(
            member.provider_intent.value().unwrap().operation,
            "transfer:agent:thread-start"
        );
        assert!(member.provider_receipts.is_missing());

        let receipt = json!({"operation":"transfer:agent:thread-start", "scope":{"account":"target","connection":"connection-1","generation":1}, "certainty":"written", "ticket":{"id":9}});
        let result = Some(json!({"thread":{"id":"target-thread"}}));
        let recorded = submitted
            .provider_receipt(
                "agent",
                "transfer:agent:thread-start",
                &test_dispatch(receipt.clone(), result.clone(), RequestCertainty::Written),
            )
            .expect("test fixture operation succeeds");
        let recorded = after(recorded);
        assert_eq!(
            recorded.operation.members.get("agent").unwrap().phase,
            Phase::Ready
        );
        assert_eq!(
            serde::Serialize::serialize(
                &(&recorded
                    .operation
                    .members
                    .get("agent")
                    .unwrap()
                    .provider_receipts
                    .value()
                    .unwrap()[0]),
                serde_json::value::Serializer
            )
            .unwrap(),
            json!({"operation":"transfer:agent:thread-start", "scope":{"account":"target","connection":"connection-1","generation":1},
                "certainty":"written", "ticket":{"generation":1,"id":9,"method":"thread/start"},
                "notSentReason":null,"result":{"thread":{"id":"target-thread"}}})
        );
        let replay = recorded
            .provider_receipt(
                "agent",
                "transfer:agent:thread-start",
                &test_dispatch(receipt, result, RequestCertainty::Written),
            )
            .expect("test fixture operation succeeds");
        assert!(!replay.changed);
    }

    #[test]
    fn adoption_requires_saved_written_receipt_and_validated_target_settings() {
        let reading = after(
            snapshot()
                .begin_read("agent")
                .expect("test fixture operation succeeds"),
        );
        let reading = after(
            reading
                .prepared_member("agent", json!({"model":"model-b"}), None)
                .expect("test fixture operation succeeds"),
        );
        let submitted = after(
            reading
                .provider_intent(
                    "agent",
                    "transfer:agent:thread-start",
                    json!({"account":"target","connection":"connection-1","generation":1}),
                    "thread/start",
                    json!({"model":"model-b"}),
                )
                .expect("test fixture operation succeeds"),
        );
        let ready = after(submitted.provider_receipt("agent", "transfer:agent:thread-start", &test_dispatch(json!({"operation":"transfer:agent:thread-start", "scope":{"account":"target","connection":"connection-1","generation":1}, "certainty":"written", "ticket":{"id":11}}), Some(json!({"thread":{"id":"target-thread"}})), RequestCertainty::Written)).expect("test fixture operation succeeds"));
        for original in [json!("legacy"), json!({"legacy":true})] {
            let mut varied = Snapshot {
                operation: ready.operation.clone(),
                agents: ready.agents.clone(),
            };
            varied.agents.get_mut("agent").unwrap().account_history =
                studio_runtime_schema::queue_command::Field::Value(
                    studio_runtime_store::StoredTransferValue::Unreadable(original.clone()),
                );
            let adopted = varied
                .adopt_receipt("agent")
                .expect("legacy history is preserved");
            let history = adopted.agents["agent"].value()["accountHistory"].clone();
            assert_eq!(history[0], original);
            assert_eq!(history[1]["targetThreadId"], "target-thread");
        }
        for account in [None, Some(Value::Null), Some(json!("alternate"))] {
            let mut settings = json!({"model":"model-b", "updatedAt":123.0});
            if let Some(account) = account {
                settings["accountKey"] = account;
            }
            let mut varied = Snapshot {
                operation: ready.operation.clone(),
                agents: ready.agents.clone(),
            };
            varied
                .operation
                .members
                .get_mut("agent")
                .unwrap()
                .target_settings = studio_runtime_schema::queue_command::Field::Value(
                serde::Deserialize::deserialize(settings.clone()).unwrap(),
            );
            let adopted = varied.adopt_receipt("agent").unwrap();
            let agent = adopted.agents.get("agent").unwrap().value();
            assert_eq!(
                agent["accountKey"],
                settings
                    .get("accountKey")
                    .and_then(Value::as_str)
                    .unwrap_or("target")
            );
            assert_eq!(agent["updatedAt"], json!(123.0));
        }
        for (path, invalid) in [
            ("/targetSettings", Value::Null),
            ("/result", Value::Null),
            ("/providerReceipts", json!([])),
            ("/providerReceipts/0/certainty", json!("unknown")),
            ("/providerReceipts/0/operation", json!("other-operation")),
            (
                "/providerReceipts/0/scope/connection",
                json!("other-connection"),
            ),
            ("/providerReceipts/0/ticket", Value::Null),
            ("/source/epoch", json!(99)),
            ("/source/threadId", json!("other-thread")),
            ("/source/cwd", json!("/other-workspace")),
        ] {
            let mut operation =
                serde::Serialize::serialize(&(&ready.operation), serde_json::value::Serializer)
                    .unwrap();
            *operation["members"]["agent"]
                .pointer_mut(path)
                .expect("fixture field") = invalid;
            let changed = Snapshot::read(
                &operation.to_string(),
                ready
                    .agents
                    .iter()
                    .map(|(id, agent)| (id.clone(), serde_json::to_string(agent).unwrap())),
            )
            .expect("typed modified snapshot");
            assert!(
                changed.adopt_receipt("agent").is_err(),
                "adopted invalid {path}"
            );
        }
        let adopted = after(
            ready
                .adopt_receipt("agent")
                .expect("test fixture operation succeeds"),
        );
        assert_eq!(
            adopted.operation.members.get("agent").unwrap().phase,
            Phase::Completed
        );
        assert_eq!(
            adopted.agent("agent").unwrap().thread_id.as_deref(),
            Some("target-thread")
        );
        assert_eq!(
            adopted.agent("agent").unwrap().model.as_deref(),
            Some("model-b")
        );
        assert_eq!(
            adopted.agent("agent").unwrap().value()["accountHistory"][0]["targetThreadId"],
            "target-thread"
        );
    }

    #[test]
    fn not_sent_releases_retry_with_same_operation_and_unknown_stays_held() {
        let first = after(intent(&snapshot()));
        let receipt = json!({"operation":"transfer:agent:thread-start", "scope":{"account":"target","connection":"connection-1","generation":1}, "certainty":"not_sent", "ticket":{"generation":7,"id":10,"method":"thread/start"}});
        let released = after(
            first
                .provider_receipt(
                    "agent",
                    "transfer:agent:thread-start",
                    &test_dispatch(receipt, None, RequestCertainty::NotSent),
                )
                .expect("test fixture operation succeeds"),
        );
        assert_eq!(
            released.operation.members.get("agent").unwrap().phase,
            Phase::Lazy
        );
        let retry = after(intent(&released));
        assert_eq!(
            retry
                .operation
                .members
                .get("agent")
                .unwrap()
                .provider_intent
                .value()
                .unwrap()
                .operation,
            "transfer:agent:thread-start"
        );
        assert_eq!(
            retry
                .operation
                .members
                .get("agent")
                .unwrap()
                .provider_receipts
                .value()
                .unwrap()[0]
                .ticket
                .value()
                .unwrap()
                .id,
            studio_runtime_providers::RequestId::Number(10)
        );

        let unknown = after(retry.provider_receipt("agent", "transfer:agent:thread-start", &test_dispatch(json!({"operation":"transfer:agent:thread-start", "scope":{"account":"target","connection":"connection-1","generation":1}, "certainty":"unknown", "ticket":null}), None, RequestCertainty::Unknown)).expect("test fixture operation succeeds"));
        assert_eq!(
            unknown.operation.members.get("agent").unwrap().phase,
            Phase::Unknown
        );
        assert!(
            unknown
                .provider_intent(
                    "agent",
                    "transfer:agent:thread-start",
                    json!({"account":"target","connection":"connection-1","generation":1}),
                    "thread/start",
                    json!({"model":"model-a"}),
                )
                .is_err()
        );
    }

    #[test]
    fn rejected_response_cannot_adopt_a_thread() {
        let submitted = after(intent(&snapshot()));
        let raw = json!({"operation":"transfer:agent:thread-start", "scope":{"account":"target","connection":"connection-1","generation":1},
            "ticket":{"generation":1,"id":9,"method":"thread/start"}});
        let mut receipt = test_session(raw, Some(json!({})), RequestCertainty::Written);
        let SessionOutcome::Written { response, .. } = &mut receipt.outcome else {
            panic!("written fixture")
        };
        *response = ProviderResponse::Rejected(json!({"thread":{"id":"not-a-success"}}));
        let recorded = after(
            submitted
                .provider_receipt(
                    "agent",
                    "transfer:agent:thread-start",
                    &TransferDispatch::Receipt(receipt),
                )
                .expect("save rejection"),
        );
        assert_eq!(
            recorded.operation.members.get("agent").unwrap().phase,
            Phase::Unknown
        );
        assert!(
            recorded
                .operation
                .members
                .get("agent")
                .unwrap()
                .result
                .is_missing()
        );
        assert!(recorded.adopt_receipt("agent").is_err());
    }

    #[test]
    fn history_pages_beyond_the_record_limit_are_kept_out_of_the_record() {
        let mut current = after(
            snapshot()
                .begin_read("agent")
                .expect("test fixture operation succeeds"),
        );
        let scope = json!({"account":"source","connection":"conn","generation":4});
        let page = json!({"data":["x".repeat(1024 * 1024)],"nextCursor":"next"});
        for n in 0..10 {
            let op_id = format!("account-transfer:transfer:agent:turns:{n}");
            current = after(
                current
                    .provider_read_intent(
                        "agent",
                        &op_id,
                        "source",
                        scope.clone(),
                        "thread/turns/list",
                        json!({"threadId":"t","limit":20}),
                    )
                    .expect("page intent"),
            );
            let receipt = json!({"operation":op_id,"scope":scope,
                "certainty":"written","ticket":{"generation":4,"id":format!("codex:{n}"),"method":"thread/turns/list"}});
            let decision = current
                .provider_read_receipt(
                    "agent",
                    &op_id,
                    &test_session(receipt, Some(page.clone()), RequestCertainty::Written),
                )
                .expect("page receipt");
            let encoded = decision.encoded().expect("record stays under the limit");
            assert!(encoded.len() < MAX_RECORD_BYTES);
            current = after(decision);
        }
        let replay = current
            .written_read_replay_typed(
                "agent",
                current
                    .record()
                    .members
                    .get("agent")
                    .unwrap()
                    .provider_read_intents
                    .value()
                    .unwrap()
                    .get("account-transfer:transfer:agent:turns:3")
                    .unwrap(),
            )
            .expect("exact replay")
            .expect("written page replays");
        assert!(replay.result.value().unwrap().value_blob().unwrap().size > 1024 * 1024);
    }

    #[test]
    fn provider_read_journal_replays_written_and_holds_uncertain_calls() {
        let reading = after(
            snapshot()
                .begin_read("agent")
                .expect("test fixture operation succeeds"),
        );
        let intent = |snapshot: &Snapshot| {
            snapshot.provider_read_intent(
                "agent",
                "account-transfer:transfer:agent:model-list:0",
                "target",
                json!({"account":"target","connection":"conn","generation":4}),
                "model/list",
                json!({"limit":100}),
            )
        };
        let prepared = after(intent(&reading).expect("test fixture operation succeeds"));
        let op_id = "account-transfer:transfer:agent:model-list:0";
        let receipt = json!({"operation":op_id,"scope":{"account":"target","connection":"conn","generation":4},
            "certainty":"written","ticket":{"generation":4,"id":"claude:4","method":"model/list"}});
        let response = json!({"kind":"reply","value":{"data":[],"nextCursor":null}});
        let recorded = after(
            prepared
                .provider_read_receipt(
                    "agent",
                    op_id,
                    &test_session(
                        receipt.clone(),
                        Some(response.clone()),
                        RequestCertainty::Written,
                    ),
                )
                .expect("test fixture operation succeeds"),
        );
        assert_eq!(
            serde_json::from_str::<Value>(
                recorded
                    .operation
                    .members
                    .get("agent")
                    .unwrap()
                    .provider_read_intents
                    .value()
                    .unwrap()[op_id]
                    .params
                    .get()
            )
            .unwrap()["limit"],
            100
        );
        assert!(
            !intent(&recorded)
                .expect("test fixture operation succeeds")
                .changed
        );
        assert_eq!(
            serde::Serialize::serialize(
                &(&recorded
                    .operation
                    .members
                    .get("agent")
                    .unwrap()
                    .provider_read_receipts
                    .value()
                    .unwrap()[op_id]
                    .result),
                serde_json::value::Serializer
            )
            .unwrap(),
            response
        );

        let not_sent = after(
            recorded
                .provider_read_intent(
                    "agent",
                    "account-transfer:transfer:agent:thread-read",
                    "source",
                    json!({"account":"source","connection":"source-conn","generation":2}),
                    "thread/read",
                    json!({"threadId":"source-thread","includeTurns":false}),
                )
                .expect("test fixture operation succeeds"),
        );
        let read_op = "account-transfer:transfer:agent:thread-read";
        let not_sent_receipt = json!({"operation":read_op,"scope":{"account":"source","connection":"source-conn","generation":2},
            "certainty":"not_sent","ticket":{"generation":2,"id":10,"method":"thread/read"}});
        let released = after(
            not_sent
                .provider_read_receipt(
                    "agent",
                    read_op,
                    &test_session(not_sent_receipt, None, RequestCertainty::NotSent),
                )
                .expect("test fixture operation succeeds"),
        );
        assert!(
            !intent(&released)
                .expect("test fixture operation succeeds")
                .changed
        );
        let retry = released
            .provider_read_intent(
                "agent",
                read_op,
                "source",
                json!({"account":"source","connection":"source-conn","generation":2}),
                "thread/read",
                json!({"threadId":"source-thread","includeTurns":false}),
            )
            .expect("NotSent allows one retry under the saved operation identity");
        assert!(!retry.changed);
        assert_eq!(
            retry
                .operation
                .members
                .get("agent")
                .unwrap()
                .provider_read_intents
                .value()
                .unwrap()[read_op]
                .operation,
            read_op
        );
        assert_eq!(
            released
                .operation
                .members
                .get("agent")
                .unwrap()
                .provider_read_receipts
                .value()
                .unwrap()[read_op]
                .receipt
                .certainty,
            studio_runtime_store::transfer_record::ReceiptCertainty::NotSent
        );
        assert_eq!(
            released
                .operation
                .members
                .get("agent")
                .unwrap()
                .provider_read_receipts
                .value()
                .unwrap()[read_op]
                .receipt
                .ticket
                .value()
                .unwrap()
                .id,
            studio_runtime_providers::RequestId::Number(10)
        );

        let unknown = after(
            released
                .provider_read_intent(
                    "agent",
                    "account-transfer:transfer:agent:turns:0",
                    "source",
                    json!({"account":"source","connection":"source-conn","generation":2}),
                    "thread/turns/list",
                    json!({"threadId":"source-thread"}),
                )
                .expect("test fixture operation succeeds"),
        );
        let unknown_op = "account-transfer:transfer:agent:turns:0";
        let unknown_receipt = json!({"operation":unknown_op,"scope":{"account":"source","connection":"source-conn","generation":2},
            "certainty":"unknown","ticket":{"generation":2,"id":11,"method":"thread/turns/list"}});
        let held = after(
            unknown
                .provider_read_receipt(
                    "agent",
                    unknown_op,
                    &test_session(unknown_receipt, None, RequestCertainty::Unknown),
                )
                .expect("test fixture operation succeeds"),
        );
        assert_eq!(
            held.operation.members.get("agent").unwrap().phase,
            Phase::Unknown
        );
        assert!(intent(&held).is_err());
    }

    #[test]
    fn intent_without_receipt_may_ask_the_retained_ledger_again() {
        let reading = after(snapshot().begin_read("agent").expect("fixture"));
        let params = json!({"threadId":"source-thread"});
        let scope = json!({"account":"source","connection":"source-conn","generation":2});
        let operation = "transfer:transfer:agent:unsubscribe";
        let ask = |snapshot: &Snapshot| {
            snapshot.provider_read_intent(
                "agent",
                operation,
                "source",
                scope.clone(),
                "thread/unsubscribe",
                params.clone(),
            )
        };
        let intent = after(ask(&reading).expect("fixture"));
        assert!(
            ask(&intent).is_ok(),
            "the retained ledger replays or holds the call by its key"
        );
    }

    #[test]
    fn source_queue_preflight_requires_transfer_scope_and_exact_receipt() {
        let reading = after(
            snapshot()
                .begin_read("agent")
                .expect("test fixture operation succeeds"),
        );
        let params = json!({"threadId":"source-thread"});
        let scope = json!({"account":"source","connection":"source-conn","generation":2});
        assert!(
            reading
                .provider_read_intent(
                    "agent",
                    "untrusted:queue",
                    "source",
                    scope.clone(),
                    "thread/queue/list",
                    params.clone(),
                )
                .is_err()
        );
        let operation = "transfer:transfer:agent:queue-before";
        let intent = after(
            reading
                .provider_read_intent(
                    "agent",
                    operation,
                    "source",
                    scope.clone(),
                    "thread/queue/list",
                    params.clone(),
                )
                .expect("test fixture operation succeeds"),
        );
        assert_eq!(
            serde_json::from_str::<Value>(
                intent
                    .operation
                    .members
                    .get("agent")
                    .unwrap()
                    .provider_read_intents
                    .value()
                    .unwrap()[operation]
                    .params
                    .get()
            )
            .unwrap(),
            params
        );
        let receipt = json!({"operation":operation,"scope":scope,"certainty":"written",
            "ticket":{"generation":2,"id":"queue-ticket","method":"thread/queue/list"}});
        let response = json!({"data":[],"nextCursor":null});
        let recorded = after(
            intent
                .provider_read_receipt(
                    "agent",
                    operation,
                    &test_session(
                        receipt,
                        Some(json!({"kind":"reply","value":response})),
                        RequestCertainty::Written,
                    ),
                )
                .expect("test fixture operation succeeds"),
        );
        assert_eq!(
            recorded.operation.members.get("agent").unwrap().phase,
            Phase::Reading
        );
        assert_eq!(
            serde_json::from_str::<Value>(
                recorded
                    .operation
                    .members
                    .get("agent")
                    .unwrap()
                    .provider_read_receipts
                    .value()
                    .unwrap()[operation]
                    .result
                    .value()
                    .unwrap()
                    .value()
                    .unwrap()
                    .get()
            )
            .unwrap()["data"],
            json!([])
        );
    }
}
