//! Closed persisted transfer records. Member order is part of the replay contract.
use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::fmt;
use studio_runtime_schema::account_key::AccountKey;
use studio_runtime_schema::queue_command::Field;

/// A JSON object whose insertion order survives decode and encode.
#[derive(Clone, Debug)]
pub struct OrderedMembers<T>(pub Vec<(String, T)>);
impl<T> OrderedMembers<T> {
    pub fn get(&self, id: &str) -> Option<&T> {
        self.0
            .iter()
            .find(|(key, _)| key == id)
            .map(|(_, value)| value)
    }
    pub fn get_mut(&mut self, id: &str) -> Option<&mut T> {
        self.0
            .iter_mut()
            .find(|(key, _)| key == id)
            .map(|(_, value)| value)
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for OrderedMembers<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OrderedVisitor<T>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for OrderedVisitor<T> {
            type Value = OrderedMembers<T>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("ordered transfer members")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut members = Vec::new();
                while let Some((id, value)) = map.next_entry::<String, T>()? {
                    if members.iter().any(|(old, _)| old == &id) {
                        return Err(serde::de::Error::custom("duplicate transfer member"));
                    }
                    members.push((id, value));
                }
                Ok(OrderedMembers(members))
            }
        }
        deserializer.deserialize_map(OrderedVisitor(std::marker::PhantomData))
    }
}
impl<T: Serialize> Serialize for OrderedMembers<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (id, member) in &self.0 {
            map.serialize_entry(id, member)?;
        }
        map.end()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferPhase {
    Waiting,
    Lazy,
    Reading,
    Interrupting,
    Submitted,
    LazySubmitted,
    Ready,
    Blocked,
    Unknown,
    Completed,
    Left,
}
impl TransferPhase {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Left)
    }
    pub fn moving(self) -> bool {
        matches!(
            self,
            Self::Reading | Self::Interrupting | Self::Submitted | Self::LazySubmitted
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TransferStatus {
    Pending,
    Completed,
    Cancelled,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TransferScope {
    Team,
    Subagents,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InterruptOutcome {
    Acknowledged,
    Unknown,
    Rejected,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransferSource {
    pub account_key: AccountKey,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub thread_id: Field<String>,
    pub epoch: i64,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub cwd: Field<String>,
}

/// Native provider payloads remain opaque and retain their exact JSON bytes.
pub type ProviderPayload = Box<serde_json::value::RawValue>;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptCertainty {
    Written,
    Unknown,
    NotSent,
}
macro_rules! transfer_methods {
    ($($variant:ident => $wire:literal),+ $(,)?) => {
        #[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
        pub enum TransferMethod { $(#[serde(rename = $wire)] $variant),+ }
        impl TransferMethod {
            pub const fn as_str(&self) -> &'static str {
                match self { $(Self::$variant => $wire),+ }
            }
        }
    };
}
transfer_methods! {
    Start => "thread/start",
    Fork => "thread/fork",
    ModelList => "model/list",
    ThreadRead => "thread/read",
    TurnsList => "thread/turns/list",
    QueueList => "thread/queue/list",
    BackgroundList => "thread/backgroundTerminals/list",
    Unsubscribe => "thread/unsubscribe",
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderIntent {
    pub operation: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub account: Field<AccountKey>,
    pub scope: studio_runtime_schema::provider_call_contract::ConnectionScope,
    pub method: TransferMethod,
    pub params: ProviderPayload,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderTicket {
    pub generation: u64,
    pub id: studio_runtime_schema::provider_input::RequestId,
    pub method: TransferMethod,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderReceipt {
    pub operation: String,
    pub scope: studio_runtime_schema::provider_call_contract::ConnectionScope,
    pub certainty: ReceiptCertainty,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub ticket: Field<ProviderTicket>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub not_sent_reason: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub result: Field<ProviderPayload>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PayloadBlob {
    pub sha256: String,
    pub size: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseKind {
    Reply,
    Rejected,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderResult {
    pub kind: ResponseKind,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub value: Field<ProviderPayload>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub value_blob: Field<PayloadBlob>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderReadReceipt {
    pub certainty: ReceiptCertainty,
    pub receipt: ProviderReceipt,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub result: Field<ProviderResult>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransferClaudeOptions {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub auto_compact_window: Field<i64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub binary_path: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub config_dir: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub custom_models: Field<Vec<ClaudeCustomModel>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub launch_args: Field<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaudeCustomModel {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransferSettings {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub provider: Field<studio_runtime_schema::ProviderKind>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub model: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub effort: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub native_effort: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub fast_mode: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub daybreak_enabled: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub cyber_access_program: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub worker_defaults: Field<TransferWorkerDefaults>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub claude_options: Field<TransferClaudeOptions>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub role: Field<studio_runtime_schema::creation_intent::AgentRole>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub profile_instructions: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub yolo_mode: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub portable_history: Field<PortableHistory>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistorySource {
    pub agent_id: String,
    pub account_key: AccountKey,
    pub epoch: Option<i64>,
    pub thread_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableHistory {
    pub version: u64,
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    pub counts: BTreeMap<String, u64>,
    pub source: HistorySource,
    pub transfer_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceEmptyProof {
    pub origin: String,
    pub transfer_id: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub thread_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub native_history: Field<EmptyNativeHistory>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransportTicket {
    pub operation: String,
    pub agent: String,
    pub epoch: i64,
    pub account: AccountKey,
    pub connection: String,
    pub request: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderAdmission {
    pub root_request_id: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub admitted_request: Field<studio_runtime_schema::persistence::AdmittedRequest>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransferMember {
    pub phase: TransferPhase,
    pub source_account_key: AccountKey,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub source_thread_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub target_thread_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub provider: Field<studio_runtime_schema::ProviderKind>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub name: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub lazy: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub waiting: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub error: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub reason: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub continue_after_transfer: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub archive_invalidated: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub archive_source_thread: Field<ProviderPayload>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub interrupt_outcome: Field<InterruptOutcome>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub interrupt_reason: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub interrupt_turn_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub interrupt_submitted_at: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub interrupt_confirmed_at: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub source: Field<TransferSource>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub settings: Field<TransferSettings>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub target_settings: Field<TransferSettings>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub pending_settings: Field<TransferSettings>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub source_pending_settings: Field<TransferSettings>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub pending_settings_account_key: Field<AccountKey>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub source_claude_options: Field<TransferClaudeOptions>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub submitted_at: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub target_connection: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub next_check: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub native_method: Field<TransferMethod>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub native_params: Field<ProviderPayload>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub result: Field<ProviderPayload>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub transport_ticket: Field<TransportTicket>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub provider_intent: Field<ProviderIntent>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub provider_admission: Field<ProviderAdmission>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub provider_read_intents: Field<BTreeMap<String, ProviderIntent>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub provider_read_admissions: Field<BTreeMap<String, ProviderAdmission>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub provider_read_receipts: Field<BTreeMap<String, ProviderReadReceipt>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub provider_receipts: Field<Vec<ProviderReceipt>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub portable_history: Field<PortableHistory>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub source_empty_proof: Field<SourceEmptyProof>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmptyNativeHistory {
    pub thread_id: String,
    pub native_turns: u64,
    pub native_items: u64,
    pub native_realtime_items: u64,
    pub rollout_missing: bool,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransferNotice {
    pub id: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub name: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub provider: Field<studio_runtime_schema::ProviderKind>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub reason: Field<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedTransferMember {
    pub phase: TransferPhase,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub source_account_key: Field<AccountKey>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub source_thread_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub target_thread_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub provider: Field<studio_runtime_schema::ProviderKind>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub name: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub reason: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub error: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub waiting: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub lazy: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub interrupt_reason: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub interrupt_outcome: Field<InterruptOutcome>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub continue_after_transfer: Field<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedTransferResponse {
    pub id: String,
    pub lead_id: String,
    pub target_account_key: AccountKey,
    pub status: TransferStatus,
    pub created: serde_json::Number,
    pub scope: TransferScope,
    pub members: BTreeMap<String, SavedTransferMember>,
    pub total: usize,
    pub completed: usize,
    pub moved: usize,
    pub native_history_pending: usize,
    pub moving_now: usize,
    pub can_finish_history: bool,
    pub waiting_count: usize,
    pub needs_attention: bool,
    pub can_retry: bool,
    pub interrupted: Vec<TransferNotice>,
    pub left_on_source: Vec<TransferNotice>,
    pub blocked: Vec<TransferNotice>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub target_provider: Field<studio_runtime_schema::ProviderKind>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub updated: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub finish_history: Field<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransferRequestIdentity {
    pub lead_id: String,
    pub target_account_key: AccountKey,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub scope: Field<TransferScope>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub saved_response: Field<SavedTransferResponse>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransferRecord {
    pub id: String,
    pub lead_id: String,
    pub target_account_key: AccountKey,
    pub status: TransferStatus,
    pub created: serde_json::Number,
    pub members: OrderedMembers<TransferMember>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub target_provider: Field<studio_runtime_schema::ProviderKind>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub updated: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub scope: Field<TransferScope>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub finish_history: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub requests: Field<BTreeMap<String, TransferRequestIdentity>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferRecordError {
    ConflictingRequest,
    UnknownRequest,
}

impl TransferRecord {
    pub fn new_team(
        id: String,
        lead_id: String,
        target: AccountKey,
        provider: studio_runtime_schema::ProviderKind,
        created: serde_json::Number,
        members: OrderedMembers<TransferMember>,
    ) -> Self {
        let status = if members.0.iter().all(|(_, member)| member.phase.terminal()) {
            TransferStatus::Completed
        } else {
            TransferStatus::Pending
        };
        let request = TransferRequestIdentity {
            lead_id: lead_id.clone(),
            target_account_key: target.clone(),
            scope: Field::Value(TransferScope::Team),
            saved_response: Field::Missing,
        };
        Self {
            id: id.clone(),
            lead_id,
            target_account_key: target,
            status,
            created,
            members,
            target_provider: Field::Value(provider),
            updated: Field::Missing,
            scope: Field::Value(TransferScope::Team),
            finish_history: Field::Missing,
            requests: Field::Value(BTreeMap::from([(id, request)])),
        }
    }

    pub fn add_team_request(
        &mut self,
        id: String,
        lead_id: String,
        target: AccountKey,
    ) -> Result<(), TransferRecordError> {
        if self.lead_id != lead_id
            || self.target_account_key != target
            || matches!(self.scope.value(), Some(TransferScope::Subagents))
        {
            return Err(TransferRecordError::ConflictingRequest);
        }
        let requests = match &mut self.requests {
            Field::Value(requests) => requests,
            Field::Missing | Field::Null => {
                self.requests = Field::Value(BTreeMap::new());
                match &mut self.requests {
                    Field::Value(requests) => requests,
                    _ => unreachable!(),
                }
            }
        };
        if let Some(saved) = requests.get(&id) {
            if saved.lead_id != lead_id
                || saved.target_account_key != target
                || !matches!(saved.scope.value(), None | Some(TransferScope::Team))
            {
                return Err(TransferRecordError::ConflictingRequest);
            }
            return Ok(());
        }
        requests.insert(
            id,
            TransferRequestIdentity {
                lead_id,
                target_account_key: target,
                scope: Field::Value(TransferScope::Team),
                saved_response: Field::Missing,
            },
        );
        Ok(())
    }

    pub fn save_response(
        &mut self,
        request_id: &str,
        saved: SavedTransferResponse,
    ) -> Result<(), TransferRecordError> {
        let request = match &mut self.requests {
            Field::Value(requests) => requests.get_mut(request_id),
            _ => None,
        }
        .ok_or(TransferRecordError::UnknownRequest)?;
        request.saved_response = Field::Value(saved);
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum AgentTransferState {
    Unreadable(UnreadableTransferSummary),
    Summary(AgentTransferSummary),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UnreadableTransferSummary {
    pub kind: UnreadableTransferKind,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UnreadableTransferKind {
    Unreadable,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTransferSummary {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub id: Field<TransferId>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub target_account_key: Field<AccountKey>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub status: Field<TransferStatus>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub updated: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub scope: Field<TransferScope>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub finish_history: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub total: Field<usize>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub completed: Field<usize>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub moved: Field<usize>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub native_history_pending: Field<usize>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub moving_now: Field<usize>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub can_finish_history: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub waiting_count: Field<usize>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub needs_attention: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub can_retry: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub can_cancel: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub interrupted: Field<Vec<TransferNotice>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub left_on_source: Field<Vec<TransferNotice>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub blocked: Field<Vec<TransferNotice>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub waiting: Field<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransferWorkerDefaults {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub model: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub effort: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub fast_mode: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub daybreak_enabled: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub account_key: Field<AccountKey>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub delegation_depth: Field<i64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub cyber_access_program: Field<String>,
}

/// Stable identity of an account transfer, shared with its lazy hold.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct TransferId(studio_runtime_schema::bounded_text::BoundedText<1, 200>);
impl TransferId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
impl TryFrom<String> for TransferId {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(Self(value.try_into()?))
    }
}

/// The pending lazy hold, or a migrated record that could not be decoded.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum LazyTransferState {
    Unreadable(UnreadableTransferSummary),
    Pending(LazyTransfer),
}
impl LazyTransferState {
    pub fn pending(&self) -> Option<&LazyTransfer> {
        match self {
            Self::Pending(value) => Some(value),
            Self::Unreadable(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LazyTransfer {
    pub id: TransferId,
    pub source_account_key: AccountKey,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub source_thread_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub source_state: Field<LazySourceState>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub source_state_exact: Field<bool>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LazySourceState {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub account_key: Field<AccountKey>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub provider: Field<studio_runtime_schema::ProviderKind>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub model: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub effort: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub native_effort: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub fast_mode: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub daybreak_enabled: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub cyber_access_program: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub worker_defaults: Field<TransferWorkerDefaults>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub pending_settings: Field<TransferSettings>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub pending_settings_account_key: Field<AccountKey>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub claude_options: Field<TransferClaudeOptions>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub execution_settings_account_key: Field<AccountKey>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub status: Field<crate::AgentStatus>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub error: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub native_failure_hold: Field<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_record_keeps_request_identity_and_terminal_status() {
        let member: TransferMember = serde_json::from_value(
            serde_json::json!({"phase":"completed","sourceAccountKey":"source"}),
        )
        .unwrap();
        let target: AccountKey = "target".to_owned().try_into().unwrap();
        let mut record = TransferRecord::new_team(
            "move".into(),
            "lead".into(),
            target.clone(),
            studio_runtime_schema::ProviderKind::Codex,
            1.into(),
            OrderedMembers(vec![("lead".into(), member)]),
        );
        assert_eq!(record.status, TransferStatus::Completed);
        assert_eq!(record.requests.value().unwrap().len(), 1);
        record
            .add_team_request("retry".into(), "lead".into(), target.clone())
            .unwrap();
        record
            .add_team_request("retry".into(), "lead".into(), target.clone())
            .unwrap();
        assert_eq!(record.requests.value().unwrap().len(), 2);
        assert_eq!(
            record.add_team_request("retry".into(), "other".into(), target),
            Err(TransferRecordError::ConflictingRequest)
        );
    }

    #[test]
    fn agent_transfer_pointers_are_owned_by_typed_fields() {
        let raw = serde_json::json!({"id":"lead","rootId":"lead","epoch":1,"status":"idle","accountKey":"source","compactions":0,
            "accountTransferId":"move", "lazyAccountTransfer":{"id":"move","sourceAccountKey":"source","sourceThreadId":null,"sourceState":{},"sourceStateExact":true}});
        let agent = crate::AgentRecord::from_value(raw.clone()).unwrap();
        assert!(!agent.extra.contains_key("accountTransferId"));
        assert!(!agent.extra.contains_key("lazyAccountTransfer"));
        assert_eq!(agent.value()["accountTransferId"], raw["accountTransferId"]);
        assert_eq!(
            agent.value()["lazyAccountTransfer"],
            raw["lazyAccountTransfer"]
        );
    }

    #[test]
    fn ordered_members_and_null_fields_round_trip() {
        let json = r#"{"id":"transfer","leadId":"lead","targetAccountKey":"target","status":"pending","created":1.0,"members":{"z":{"phase":"waiting","sourceAccountKey":"source","sourceThreadId":null},"a":{"phase":"lazy","sourceAccountKey":"source"}}}"#;
        let record: TransferRecord = serde_json::from_str(json).unwrap();
        assert_eq!(
            record
                .members
                .0
                .iter()
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>(),
            vec!["z", "a"]
        );
        assert!(matches!(
            record.members.0[0].1.source_thread_id,
            Field::Null
        ));
        assert!(matches!(
            record.members.0[1].1.source_thread_id,
            Field::Missing
        ));
        let encoded = serde_json::to_string(&record).unwrap();
        assert!(encoded.find("\"z\"").unwrap() < encoded.find("\"a\"").unwrap());
        assert!(encoded.contains("\"sourceThreadId\":null"));
    }

    #[test]
    fn unknown_member_field_is_rejected() {
        let json = r#"{"id":"transfer","leadId":"lead","targetAccountKey":"target","status":"pending","created":1.0,"members":{"lead":{"phase":"waiting","sourceAccountKey":"source","unexpected":1}}}"#;
        assert!(serde_json::from_str::<TransferRecord>(json).is_err());
    }

    #[test]
    #[ignore = "run explicitly against a read-only copied corpus"]
    fn copied_transfer_corpus_decodes_without_losing_fields() {
        fn difference(left: &serde_json::Value, right: &serde_json::Value) -> String {
            match (left, right) {
                (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
                    for (key, value) in a {
                        if b.get(key) != Some(value) {
                            let label = if key.len() > 20 { "*" } else { key };
                            return format!(
                                "/{label}{}",
                                b.get(key)
                                    .map_or_else(String::new, |other| difference(value, other))
                            );
                        }
                    }
                    "/extra".into()
                }
                (serde_json::Value::Array(a), serde_json::Value::Array(b)) => {
                    for (index, (left, right)) in a.iter().zip(b).enumerate() {
                        if left != right {
                            return format!("/{index}{}", difference(left, right));
                        }
                    }
                    "/length".into()
                }
                _ => String::new(),
            }
        }
        let path = std::env::var("STUDIO_TRANSFER_CORPUS").expect("copied corpus path");
        let connection =
            rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
        let mut statement = connection
            .prepare("SELECT record FROM runtime_account_transfers ORDER BY rowid")
            .unwrap();
        let records = statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap();
        for (index, raw) in records.enumerate() {
            let raw = raw.unwrap();
            let record: TransferRecord = serde_json::from_str(&raw)
                .unwrap_or_else(|error| panic!("record {index}: {error}"));
            let original: serde_json::Value = serde_json::from_str(&raw).unwrap();
            let encoded = serde_json::to_string(&record).unwrap();
            let restored: serde_json::Value = serde_json::from_str(&encoded).unwrap();
            assert!(
                original == restored,
                "record {index} changed at {}",
                difference(&original, &restored)
            );
        }
    }
}
