//! Logical input to the existing transaction journal, never a second publisher.
use studio_runtime_schema::account_key::AccountKey;
use studio_runtime_schema::persistence::HoldStatus;

use super::{AgentComponents, AgentIdentity, DeletedAtEvidence};

/// Which logical component changed; this is not a stored table partition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentComponent {
    Agent,
    Chat,
    Live,
}

/// Capture before a row can disappear or its physical slot can be reused.
/// Absent components have absent owners; missing selectors are not defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentOwnerSnapshot {
    pub identity: AgentIdentity,
    pub agent_id: Option<crate::AgentId>,
    pub components: Option<AgentComponents>,
    pub live_present: bool,
    pub account_key: Option<AccountKey>,
    pub parent_id: Option<crate::AgentId>,
    pub parent_account_key: Option<AccountKey>,
    pub root_id: Option<crate::AgentId>,
    pub hold: Option<HoldStatus>,
    /// None is an absent agent; Some(Null) is a present, undeleted selector.
    pub deleted_at: Option<DeletedAtEvidence>,
}

/// One writer-transaction observation. Rollback discards it with other hints.
/// OLD and NEW must both survive same-transaction delete/reinsert or key moves.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentComponentChange {
    pub component: AgentComponent,
    /// A lookup hint only; never bind this as a legacy source-order column.
    pub physical_rowid: Option<i64>,
    pub before: Option<AgentOwnerSnapshot>,
    pub after: Option<AgentOwnerSnapshot>,
}
