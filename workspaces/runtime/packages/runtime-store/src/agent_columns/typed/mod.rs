//! Typed agent component contracts, separate from the legacy schema installers.
//!
//! Declaring these types does not register a target or change writer authority.
mod agent_table;
mod codecs;
mod components;
mod evidence;
mod identity;
mod journal;
pub(crate) mod record;
pub(crate) mod retained;
mod session_table;
mod turn_attempt;
mod turn_table;
pub use agent_table::TypedAgentRow;
pub use codecs::*;

pub use components::{
    AdmittedAgentSource, AgentComponents, AgentSourceSnapshot, HeldAgentEvidence,
    HeldAgentSelectors,
};
pub use evidence::*;
pub use identity::{AgentIdentity, AgentIdentityError, NullableAgentSource};
pub use journal::{AgentComponent, AgentComponentChange, AgentOwnerSnapshot};
pub use session_table::{SessionsForChat, SessionsForThread, TypedSessionRow, session_copy_plan};
pub use turn_attempt::{LegacyAttemptEvidence, StoredTurnAttempt, TurnAttemptCodec};
pub use turn_table::TypedTurnRow;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod component_tests;

#[cfg(test)]
mod codec_tests;

#[cfg(test)]
mod state_tests;

#[cfg(test)]
mod session_tests;

#[cfg(test)]
mod turn_attempt_tests;

#[cfg(test)]
mod record_evidence_tests;

#[cfg(test)]
mod turn_tests;

#[cfg(test)]
mod shared_evidence_tests;

pub(crate) mod context_repair;
