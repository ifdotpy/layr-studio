//! Borrowed native progress intents. JSON belongs to transcript persistence only.
use super::{
    DispatchOutcome, NotificationDispatchError, NotificationStreamBuffer, ProviderDeliveryIdentity,
    TextUpdate, TextWriteInput, TextWriteOutcome,
};
use std::borrow::Cow;
use studio_json::{Value, json};
use studio_runtime_providers::{Frame, codex};
use studio_runtime_store::{AgentRecord, NotificationDispatchRepository, Transaction};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Plan,
    Reasoning,
}
impl Kind {
    pub(super) fn method(self) -> &'static str {
        // The summary-part method is the canonical key for the public-summary
        // stream; the legacy text-delta counter keeps its separate key.
        match self {
            Self::Plan => codex::ServerNotificationMethod::ItemPlanDelta.as_str(),
            Self::Reasoning => {
                codex::ServerNotificationMethod::ItemReasoningSummaryPartAdded.as_str()
            }
        }
    }
    pub(super) fn from_method(method: &str) -> Option<Self> {
        if method == Self::Plan.method() {
            Some(Self::Plan)
        } else if method == Self::Reasoning.method() {
            Some(Self::Reasoning)
        } else {
            None
        }
    }
    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Plan => "Plan",
            Self::Reasoning => "Reasoning summary",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Scope<'a> {
    pub method: &'static str,
    pub thread: &'a str,
    pub turn: &'a str,
    pub item: &'a str,
}
pub(crate) struct Native<'a> {
    event: Event<'a>,
    pub scope: Scope<'a>,
}

#[derive(Clone, Copy)]
enum Event<'a> {
    Plan(&'a codex::PlanDeltaNotification),
    Summary(&'a codex::ReasoningSummaryTextDeltaNotification),
    Part(&'a codex::ReasoningSummaryPartAddedNotification),
    Tool(&'a codex::McpToolCallProgressNotification),
    Started(&'a codex::ThreadItem),
    Completed(&'a codex::ThreadItem),
}

pub(crate) fn native(frame: &Frame) -> Option<Native<'_>> {
    use codex::{ServerNotification as N, ThreadItem};
    let Frame::CodexNotification(notification) = frame else {
        return None;
    };
    use studio_runtime_providers::codex_disposition::ProgressHandler as H;
    let studio_runtime_providers::codex_disposition::Disposition::Handled(
        studio_runtime_providers::codex_disposition::HandlerKey::Notification(
            studio_runtime_providers::codex_disposition::NotificationHandler::Progress(key),
        ),
    ) = studio_runtime_providers::codex_disposition::server_notification(notification.method())
    else {
        return None;
    };
    // Both pinned payload shapes share the same borrowed routing fields.
    macro_rules! native_event {
        ($variant:ident, $event:ident) => {{
            let N::$variant { params, .. } = notification.as_ref() else {
                return None;
            };
            (
                &params.thread_id,
                &params.turn_id,
                &params.item_id,
                Event::$event(params),
            )
        }};
        ($variant:ident, $event:ident, item) => {{
            let N::$variant { params, .. } = notification.as_ref() else {
                return None;
            };
            let (ThreadItem::Plan { id, .. } | ThreadItem::Reasoning { id, .. }) = &params.item
            else {
                return None;
            };
            (
                &params.thread_id,
                &params.turn_id,
                id,
                Event::$event(&params.item),
            )
        }};
    }
    let (thread, turn, item, event) = match key {
        H::ItemPlanDelta => native_event!(ItemPlanDelta, Plan),
        H::ItemReasoningSummaryTextDelta => native_event!(ItemReasoningSummaryTextDelta, Summary),
        H::ItemReasoningSummaryPartAdded => native_event!(ItemReasoningSummaryPartAdded, Part),
        H::ItemMcpToolCallProgress => native_event!(ItemMcpToolCallProgress, Tool),
        H::ItemStarted => native_event!(ItemStarted, Started, item),
        H::ItemCompleted => native_event!(ItemCompleted, Completed, item),
    };
    Some(Native {
        event,
        scope: Scope {
            method: notification.method().as_str(),
            thread,
            turn,
            item,
        },
    })
}

impl Native<'_> {
    /// The worker uses the same borrowed scope for correlation as the text writer.
    pub(crate) fn route(&self) -> (&'static str, &str, Option<&str>) {
        (self.scope.method, self.scope.thread, Some(self.scope.turn))
    }

    pub(super) fn apply(
        &self,
        tx: &Transaction<'_>,
        agent_id: &str,
        agent: &mut AgentRecord,
        streams: &mut NotificationStreamBuffer,
        identity: &ProviderDeliveryIdentity,
        at: f64,
    ) -> Result<(DispatchOutcome, bool), NotificationDispatchError> {
        let scope = self.scope;
        super::validate_agent_write_fields(agent, agent_id)?;
        if scope.thread.is_empty()
            || scope.turn.is_empty()
            || scope.item.is_empty()
            || agent.thread_id.as_deref() != Some(scope.thread)
            || identity.provider_handle != format!("account:{}", agent.account_key)
        {
            return Err(NotificationDispatchError::invalid());
        }
        if agent.turn_id.as_deref() != Some(scope.turn)
            || super::turn_settled(agent, Some(scope.turn))
        {
            return Ok((
                DispatchOutcome::Ignored {
                    reason: "stale-turn-item".into(),
                },
                false,
            ));
        }
        let retry_recovered = !matches!(self.event, Event::Tool(_))
            && super::items::advance_reasoning_for_turn(agent, Some(scope.turn), at);
        let write =
            |agent: &mut AgentRecord, streams: &mut NotificationStreamBuffer, kind, update| {
                super::write_text_notification(
                    tx,
                    agent,
                    streams,
                    TextWriteInput {
                        agent_id,
                        account: identity
                            .provider_handle
                            .strip_prefix("account:")
                            .expect("validated account"),
                        generation: identity.generation,
                        params: &Value::Null,
                        item_id: scope.item,
                        native: Some((scope, kind)),
                        observed_at: at,
                        update,
                    },
                )
            };
        let (written, snapshot) = match self.event {
            Event::Plan(params) => (
                write(agent, streams, Kind::Plan, TextUpdate::Delta(&params.delta))?,
                false,
            ),
            Event::Summary(params) => (
                write(
                    agent,
                    streams,
                    Kind::Reasoning,
                    TextUpdate::Delta(&params.delta),
                )?,
                false,
            ),
            Event::Part(params) => {
                if params.summary_index < 0 {
                    return Err(NotificationDispatchError::invalid());
                }
                (
                    write(
                        agent,
                        streams,
                        Kind::Reasoning,
                        TextUpdate::Delta(if params.summary_index > 0 { "\n\n" } else { "" }),
                    )?,
                    false,
                )
            }
            Event::Started(item) => {
                let (kind, text) = snapshot_text(item);
                (
                    write(agent, streams, kind, TextUpdate::Started(Some(&text), None))?,
                    true,
                )
            }
            Event::Completed(item) => {
                let (kind, text) = snapshot_text(item);
                (
                    write(
                        agent,
                        streams,
                        kind,
                        TextUpdate::Completed(Some(&text), None),
                    )?,
                    true,
                )
            }
            Event::Tool(params) => return tool(tx, agent, params, at),
        };
        if snapshot && written == TextWriteOutcome::Stored {
            note_event(agent, 1, at)?;
        }
        Ok((
            if written == TextWriteOutcome::Closed {
                DispatchOutcome::Ignored {
                    reason: "closed-stream".into(),
                }
            } else {
                DispatchOutcome::Written
            },
            written == TextWriteOutcome::Stored || retry_recovered,
        ))
    }
}

fn snapshot_text(item: &codex::ThreadItem) -> (Kind, Cow<'_, str>) {
    if let codex::ThreadItem::Plan { text, .. } = item {
        (Kind::Plan, Cow::Borrowed(text))
    } else if let codex::ThreadItem::Reasoning { summary, .. } = item {
        (
            Kind::Reasoning,
            Cow::Owned(summary.as_deref().unwrap_or_default().join("\n\n")),
        )
    } else {
        unreachable!("only plan and reasoning items enter the progress projection")
    }
}

pub(super) fn note_event(
    agent: &mut AgentRecord,
    count: u64,
    at: f64,
) -> Result<(), NotificationDispatchError> {
    agent.events = Some(agent.events.unwrap_or_default().saturating_add(count));
    agent.last_event = Some(super::format_utc(at)?);
    Ok(())
}

fn tool(
    tx: &Transaction<'_>,
    agent: &mut AgentRecord,
    p: &codex::McpToolCallProgressNotification,
    at: f64,
) -> Result<(DispatchOutcome, bool), NotificationDispatchError> {
    let key = format!("{}:{}", agent.id.as_ref(), p.item_id);
    let Some((record, _)) =
        NotificationDispatchRepository::notification_item(tx, agent.id.as_ref(), &key)?
    else {
        return Ok((
            DispatchOutcome::Ignored {
                reason: "missing-tool-item".into(),
            },
            false,
        ));
    };
    if record["toolStatus"] != "running" || record["turnId"].as_str() != Some(p.turn_id.as_str()) {
        return Ok((
            DispatchOutcome::Ignored {
                reason: "closed-tool-item".into(),
            },
            false,
        ));
    }
    let item: codex::ThreadItem = studio_json::from_str(&super::saved_text(tx, &key, &record)?)
        .map_err(|_| super::stored("runtime_items", &key))?;
    if !matches!(&item, codex::ThreadItem::McpToolCall { id, .. } if *id == p.item_id) {
        return Err(super::stored("runtime_items", &key));
    }
    #[derive(serde::Serialize)]
    struct ToolProgress<'a> {
        #[serde(flatten)]
        item: &'a codex::ThreadItem,
        #[serde(rename = "progressMessage")]
        message: &'a str,
    }
    let message = super::suffix(&p.message, super::items::TOOL_OUTPUT_CHARS);
    let text = studio_json::to_string(&ToolProgress {
        item: &item,
        message: &message,
    })?;
    super::kinds::write_item(
        tx,
        agent.id.as_ref(),
        &p.item_id,
        ("output", "mcpToolCall"),
        &text,
        json!({"toolStatus":"running","turnId":p.turn_id}),
        at,
    )?;
    note_event(agent, 1, at)?;
    Ok((DispatchOutcome::Written, true))
}
