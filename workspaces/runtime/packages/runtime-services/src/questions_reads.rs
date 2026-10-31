//! Projection rules for persisted questions, complaints, and checkpoints.

use std::collections::BTreeSet;
use std::fmt;

use serde::Deserialize;
use serde::de::{MapAccess, Visitor};
use serde_json::{Value, json};
use studio_runtime_store::{AgentRecord, CheckpointRecord, ComplaintRecord};
use thiserror::Error;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum QuestionsReadsError {
    #[error("unknown agent")]
    UnknownAgent,
    #[error("this agent was deleted")]
    DeletedAgent,
    #[error("unknown complaint")]
    UnknownComplaint,
    #[error("invalid stored object {object}/{id}")]
    InvalidStoredObject { object: &'static str, id: String },
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionHistory<'a> {
    id: String,
    agent: String,
    method: studio_runtime_schema::request_record::RequestMethod,
    status: studio_runtime_schema::persistence::RequestCardStatus,
    created_at: f64,
    answered_at: Option<f64>,
    answered_by: Option<String>,
    decision: Option<studio_runtime_schema::request_record::RequestDecision>,
    deferred: Option<bool>,
    deferred_at: Option<f64>,
    deferred_by: Option<String>,
    answer_history: Option<Vec<HistoryAnswer<'a>>>,
    answer_error: Option<String>,
    item_id: Option<String>,
    turn_id: Option<String>,
    questions: Vec<QuestionView>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryAnswer<'a> {
    id: String,
    question: String,
    is_secret: bool,
    answer: Option<&'a serde_json::value::RawValue>,
}

#[derive(serde::Serialize)]
#[serde(untagged)]
enum QuestionView {
    Elicitation {
        id: String,
        question: String,
        #[serde(rename = "isSecret")]
        is_secret: bool,
    },
    Input {
        id: String,
        question: String,
        #[serde(rename = "isSecret")]
        is_secret: bool,
        header: Option<String>,
        options: Vec<QuestionOption>,
        #[serde(rename = "multiSelect")]
        multi_select: bool,
        #[serde(rename = "isOther")]
        is_other: bool,
    },
}

#[derive(serde::Serialize)]
struct QuestionOption {
    label: String,
    description: Option<String>,
}

fn history_text(value: &str, field: &str) -> String {
    crate::sync_views::bound_sync_text(value, field, false)
}
fn history_optional(value: Option<&str>, field: &str) -> Option<String> {
    value.map(|value| history_text(value, field))
}

/// Question history projection published per request.
pub(crate) fn question_history_entity(
    record: &studio_runtime_store::RequestRecord,
) -> Result<Option<QuestionHistory<'_>>, QuestionsReadsError> {
    use studio_runtime_schema::request_record::RequestMethod;
    if record.deleted_at.is_some_and(|at| at != 0.0)
        || matches!(
            record.method,
            RequestMethod::BrowserOpen
                | RequestMethod::MonitorApprove
                | RequestMethod::CommandApproval
                | RequestMethod::FileApproval
                | RequestMethod::PermissionsApproval
        )
    {
        return Ok(None);
    }
    let params: HistoryParams =
        serde_json::from_str(record.params.get()).map_err(|_| invalid("requests", &record.id))?;
    if record.deleted_at.is_some_and(|at| at != 0.0)
        || !match record.method {
            RequestMethod::AsyncQuestion | RequestMethod::UserInput => true,
            RequestMethod::Elicitation => params.mode.as_deref() == Some("form"),
            RequestMethod::BrowserOpen
            | RequestMethod::MonitorApprove
            | RequestMethod::CommandApproval
            | RequestMethod::FileApproval
            | RequestMethod::PermissionsApproval => false,
        }
    {
        return Ok(None);
    }
    let questions = if record.method == RequestMethod::Elicitation {
        params
            .requested_schema
            .as_ref()
            .and_then(|schema| schema.properties.as_ref())
            .map(|properties| {
                properties
                    .0
                    .iter()
                    .take(crate::sync_views::SYNC_ARRAY_ITEMS)
                    .map(|(id, property)| QuestionView::Elicitation {
                        id: history_text(id, "id"),
                        question: history_text(property.title.as_deref().unwrap_or(id), "question"),
                        is_secret: property.is_secret.unwrap_or(false)
                            || property.write_only.unwrap_or(false)
                            || property.format.as_deref() == Some("password"),
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        params
            .questions
            .as_ref()
            .map(|questions| {
                questions
                    .iter()
                    .take(crate::sync_views::SYNC_ARRAY_ITEMS)
                    .map(|question| QuestionView::Input {
                        id: history_text(&question.id, "id"),
                        question: history_text(
                            question.question.as_deref().unwrap_or(&question.id),
                            "question",
                        ),
                        is_secret: question.is_secret.unwrap_or(false),
                        header: history_optional(question.header.as_deref(), "header"),
                        options: question
                            .options
                            .as_ref()
                            .map(|options| {
                                options
                                    .iter()
                                    .take(crate::sync_views::SYNC_ARRAY_ITEMS)
                                    .map(|option| match option {
                                        HistoryOption::Label(label) => QuestionOption {
                                            label: history_text(label, "label"),
                                            description: None,
                                        },
                                        HistoryOption::Detailed { label, description } => {
                                            QuestionOption {
                                                label: history_text(label, "label"),
                                                description: history_optional(
                                                    description.as_deref(),
                                                    "description",
                                                ),
                                            }
                                        }
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                        multi_select: question.multi_select.unwrap_or(false),
                        is_other: question.is_other.unwrap_or(false),
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let item_id = params
        .item_id
        .as_deref()
        .filter(|v| !v.is_empty())
        .or_else(|| {
            if record.method == RequestMethod::AsyncQuestion {
                record
                    .id
                    .strip_prefix(&record.agent)
                    .and_then(|s| s.strip_prefix(ASYNC_QUESTION_SEPARATOR))
                    .filter(|s| !s.is_empty())
            } else {
                None
            }
        });
    let turn_id = params
        .turn_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .or(record.turn_id.as_deref().filter(|s| !s.is_empty()));
    let answer_history = record.answer_history.as_ref().map(|entries| {
        entries
            .iter()
            .take(crate::sync_views::SYNC_ARRAY_ITEMS)
            .map(|entry| HistoryAnswer {
                id: history_text(&entry.id, "id"),
                question: history_text(&entry.question, "question"),
                is_secret: entry.is_secret,
                answer: (!entry.is_secret).then_some(entry.answer.as_ref()),
            })
            .collect()
    });
    if record.id.is_empty() {
        return Ok(None);
    }
    Ok(Some(QuestionHistory {
        id: history_text(&record.id, "id"),
        agent: history_text(&record.agent, "agent"),
        method: record.method,
        status: record.status,
        created_at: record.created_at,
        answered_at: record.answered_at,
        answered_by: history_optional(record.answered_by.as_deref(), "answeredBy"),
        decision: record.decision,
        deferred: record.deferred,
        deferred_at: record.deferred_at,
        deferred_by: history_optional(record.deferred_by.as_deref(), "deferredBy"),
        answer_history,
        answer_error: history_optional(record.answer_error.as_deref(), "answerError"),
        item_id: history_optional(item_id, "itemId"),
        turn_id: history_optional(turn_id, "turnId"),
        questions,
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryParams {
    mode: Option<String>,
    item_id: Option<String>,
    turn_id: Option<String>,
    questions: Option<Vec<HistoryQuestion>>,
    requested_schema: Option<HistorySchema>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryQuestion {
    id: String,
    question: Option<String>,
    is_secret: Option<bool>,
    header: Option<String>,
    options: Option<Vec<HistoryOption>>,
    multi_select: Option<bool>,
    is_other: Option<bool>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum HistoryOption {
    Label(String),
    Detailed {
        label: String,
        description: Option<String>,
    },
}
#[derive(Deserialize)]
struct HistorySchema {
    properties: Option<OrderedProperties>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryProperty {
    title: Option<String>,
    is_secret: Option<bool>,
    write_only: Option<bool>,
    format: Option<String>,
}
struct OrderedProperties(Vec<(String, HistoryProperty)>);
impl<'de> Deserialize<'de> for OrderedProperties {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OrderedVisitor;
        impl<'de> Visitor<'de> for OrderedVisitor {
            type Value = OrderedProperties;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object of elicitation properties")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    entries.push(entry);
                }
                Ok(OrderedProperties(entries))
            }
        }
        deserializer.deserialize_map(OrderedVisitor)
    }
}

pub fn complaint_detail(stored: Option<ComplaintRecord>) -> Result<Value, QuestionsReadsError> {
    let record = stored.ok_or(QuestionsReadsError::UnknownComplaint)?;
    Ok(record.into_value())
}

pub fn checkpoint_list(
    agent_id: Option<&str>,
    agents: Vec<AgentRecord>,
    checkpoints: Vec<CheckpointRecord>,
) -> Result<Value, QuestionsReadsError> {
    let ids = if let Some(agent_id) = agent_id.filter(|id| !id.is_empty()) {
        let caller = agents
            .iter()
            .find(|agent| agent.id().as_ref() == agent_id)
            .ok_or(QuestionsReadsError::UnknownAgent)?;
        if caller.is_deleted() {
            return Err(QuestionsReadsError::DeletedAgent);
        }
        vec![agent_id.to_owned()]
    } else {
        agents
            .iter()
            .filter(|agent| !agent.is_deleted())
            .map(|agent| agent.id().as_ref().to_owned())
            .collect()
    };
    let ids = ids.into_iter().collect::<BTreeSet<_>>();
    let mut items = Vec::new();
    for checkpoint in checkpoints {
        if !ids.contains(checkpoint.agent().unwrap_or_default()) {
            continue;
        }
        let Value::Object(mut record) = checkpoint.into_value() else {
            unreachable!("typed checkpoint serializes as an object");
        };
        record.remove("items");
        record.remove("historyDelta");
        record.remove("_payloadBlobs");
        items.push(Value::Object(record));
    }
    Ok(json!({"checkpoints": items}))
}

/// Separator of an asynchronous question id: `<agent>:question:<agent message item>`.
const ASYNC_QUESTION_SEPARATOR: &str = ":question:";

/// Request id of the asynchronous question that a completed agent message asks.
pub fn async_question_id(agent: &str, item: &str) -> String {
    format!("{agent}{ASYNC_QUESTION_SEPARATOR}{item}")
}

fn invalid(object: &'static str, id: &str) -> QuestionsReadsError {
    QuestionsReadsError::InvalidStoredObject {
        object,
        id: id.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{QuestionsReadsError, checkpoint_list, complaint_detail};
    use serde_json::json;
    use studio_runtime_store::{AgentRecord, CheckpointRecord};

    #[test]
    fn history_retains_property_order_and_redacts_secret_answers() {
        let raw = r#"{"id":"r","agent":"a","method":"mcpServer/elicitation/request","status":"answered","createdAt":1,"params":{"mode":"form","requestedSchema":{"properties":{"z":{"title":"Last","writeOnly":true},"a":{"title":"First"}}}},"answerHistory":[{"id":"z","question":"Last","isSecret":true,"answer":"secret"}]}"#;
        let record = serde_json::from_str(raw).unwrap();
        let result = json!(super::question_history_entity(&record).unwrap().unwrap());
        assert_eq!(result["questions"][0]["id"], "z");
        assert_eq!(result["questions"][1]["id"], "a");
        assert_eq!(result["answerHistory"][0]["answer"], json!(null));
    }

    fn agent(id: &str, root: &str, deleted_at: Option<f64>) -> AgentRecord {
        let mut record = json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,"id":id,"rootId":root,"epoch":0,"status":"idle"});
        if let Some(deleted_at) = deleted_at {
            record["deletedAt"] = json!(deleted_at);
        }
        AgentRecord::from_value(record).expect("agent fixture validates")
    }

    #[test]
    fn refuses_unknown_references() {
        assert_eq!(
            complaint_detail(None),
            Err(QuestionsReadsError::UnknownComplaint)
        );
    }

    #[test]
    fn checkpoint_summaries_filter_agents_and_strip_payloads() {
        let row = json!({"id":"checkpoint","agent":"lead","created":1.0,"items":["large"],"historyDelta":["large"],"_payloadBlobs":{"secret":"sha"},"extension":9007199254740993_u64});
        let mut other = row.clone();
        other["id"] = json!("other-checkpoint");
        other["agent"] = json!("other");
        let rows = vec![row, other]
            .into_iter()
            .map(|row| CheckpointRecord::from_value(row).expect("checkpoint validates"))
            .collect();
        let result = checkpoint_list(Some("lead"), vec![agent("lead", "lead", None)], rows)
            .expect("checkpoint summary succeeds");
        assert_eq!(
            result["checkpoints"].as_array().expect("checkpoints").len(),
            1
        );
        assert_eq!(result["checkpoints"][0]["id"], "checkpoint");
        assert!(result["checkpoints"][0].get("historyDelta").is_none());
        assert!(result["checkpoints"][0].get("items").is_none());
        assert!(result["checkpoints"][0].get("_payloadBlobs").is_none());
        assert_eq!(
            result["checkpoints"][0]["extension"].as_u64(),
            Some(9_007_199_254_740_993)
        );
    }

    #[test]
    fn refuses_deleted_callers_for_checkpoint_reads() {
        let deleted = agent("deleted", "deleted", Some(1.0));
        assert_eq!(
            checkpoint_list(Some("deleted"), vec![deleted], vec![]),
            Err(QuestionsReadsError::DeletedAgent),
        );
    }
}
