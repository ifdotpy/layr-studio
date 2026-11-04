//! The client views of an approval row. `entity_scope` builds them at read time; both
//! entities carry the typed `Approval`.
//!
//! - `request`: an approval that waits for the person (Pending) or whose answer
//!   is on its way (Answering). Every other state is a deletion.
//! - `questionHistory`: every state of a question or agent question, so a
//!   transcript card shows the answer after the request left the list.
//!
//! A hidden row is deleted in both. An asynchronous question also carries
//! `detail.params.itemId`, the agent message item named by its id.
use studio_runtime_schema::approvals::{Approval, ApprovalState, Request};
use studio_runtime_store::StoreError;

/// The view of one approval and whether each collection shows it.
pub(crate) struct ApprovalView {
    pub value: serde_json::Value,
    /// Shown in `request`.
    pub open: bool,
    /// Shown in `questionHistory`.
    pub asked: bool,
}

pub(crate) fn view(row: &Approval) -> Result<ApprovalView, StoreError> {
    let visible = row.hidden.is_none();
    let open = visible
        && matches!(
            row.state,
            ApprovalState::Pending | ApprovalState::Answering { .. }
        );
    let asked = visible
        && matches!(
            row.request,
            Request::Question { .. } | Request::AgentQuestion { .. }
        );
    let mut value = serde::Serialize::serialize(row, serde_json::value::Serializer)?;
    // The transcript item that an asynchronous question belongs to is part of its id.
    if matches!(row.request, Request::AgentQuestion { .. })
        && let Some(item) =
            crate::questions_reads::async_question_item(row.agent.as_str(), row.id.as_str())
        && let Some(params) = value["detail"]["params"].as_object_mut()
    {
        params
            .entry("itemId")
            .or_insert_with(|| serde_json::Value::String(item.to_owned()));
    }
    Ok(ApprovalView { value, open, asked })
}
