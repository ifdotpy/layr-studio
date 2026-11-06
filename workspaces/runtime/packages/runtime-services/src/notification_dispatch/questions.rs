//! Async questions and exact native resolution, within the delivery transaction.
use super::notices::ordered_text;
use super::snapshot::checked;
use super::{NotificationDispatchError, stored};
use serde::Deserialize;
use studio_json::{Value, json};
use studio_runtime_schema::complaint_record::{
    ComplaintRecipient, ComplaintSource, ComplaintStatus,
};
use studio_runtime_store::CachedSql;
use studio_runtime_store::{
    AgentId, AgentRecord, AgentRepository, ComplaintRecord, Table, Transaction,
};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Question {
    title: String,
    options: Option<Vec<Value>>,
    #[serde(default, alias = "multi_select")]
    multi_select: bool,
    #[serde(default)]
    is_secret: bool,
}
fn fields(value: &Value) -> Result<Vec<Value>, NotificationDispatchError> {
    Vec::<Question>::deserialize(value).map_err(|_| NotificationDispatchError::invalid())?.into_iter().enumerate().map(|(index,q)| {
        let options=q.options.unwrap_or_default().into_iter().map(|option|match option {
            Value::String(label)=>Ok(json!({"label":label})),
            Value::Object(ref object) if object.get("label").is_some_and(Value::is_string)=>Ok(option),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_)=>Err(NotificationDispatchError::invalid()),
        }).collect::<Result<Vec<_>,_>>()?;
        Ok(json!({"id":index.to_string(),"question":q.title,"options":options,"multiSelect":q.multi_select,"isSecret":q.is_secret}))
    }).collect()
}

pub(crate) fn submit_agent_complaint(
    tx: &Transaction<'_>,
    agent: &AgentRecord,
    request_id: &str,
    text: &str,
    at: f64,
) -> Result<ComplaintRecord, NotificationDispatchError> {
    let agent_id = agent.id.as_ref();
    let lead_id = agent.root_id.as_ref();
    let lead = AgentRepository::get(tx, &AgentId::from(lead_id))?
        .ok_or_else(|| stored("runtime_agents", lead_id))?;
    if lead.is_lead != Some(true) || lead.is_deleted() {
        return Err(NotificationDispatchError::invalid());
    }
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 12_000 {
        return Err(NotificationDispatchError::invalid());
    }
    let complaint_id = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("complaint:{request_id}").as_bytes(),
    )
    .to_string();
    if let Some(previous) = Table::<ComplaintRecord>::new(tx).get(&complaint_id)? {
        if previous.text() != Some(text)
            || previous.author() != Some(agent_id)
            || previous.lead_id() != Some(lead_id)
        {
            return Err(NotificationDispatchError::ConflictingInput);
        }
        return Ok(previous);
    }
    let recipient = if agent_id == lead_id {
        ComplaintRecipient::User
    } else {
        ComplaintRecipient::Lead
    };
    let typed = ComplaintRecord {
        id: complaint_id.clone(),
        lead_id: Some(lead_id.to_owned()),
        author: Some(agent_id.to_owned()),
        text: Some(text.to_owned()),
        recipient,
        version: Some(1),
        status: ComplaintStatus::Open,
        created: studio_runtime_schema::complaint_record::ComplaintSeconds::from_seconds(at),
        updated: studio_runtime_schema::complaint_record::ComplaintSeconds::from_seconds(at),
        read_at: None,
        responses: Vec::new(),
        source_type: None,
        title: None,
    };
    let record = typed.clone().into_value();
    checked("runtime_complaints", &record)?;
    Table::<ComplaintRecord>::new(tx).put(&typed)?;
    if recipient == ComplaintRecipient::Lead {
        let complaint = ordered_text(
            &json!({"complaint_id":complaint_id,"author":agent_id,"author_name":agent.name.as_deref().unwrap_or(agent.id.as_ref()),"text":text,"status":"open","created":at}),
            [
                "complaint_id",
                "author",
                "author_name",
                "text",
                "status",
                "created",
            ]
            .map(str::to_owned),
        );
        let message = format!(
            "{{\"complaints\": [{complaint}], \"required\": {}}}",
            json!(
                "Respond to each complaint with orchestration_complaint action=respond before finishing. The full complaint is in this message. No separate read call is required."
            )
        );
        let inserted = crate::event_write::enqueue_event(
            tx,
            &lead,
            format!("complaint:{complaint_id}"),
            "complaint",
            message,
            at,
        )
        .map_err(|_| stored("runtime_events", &complaint_id))?;
        if inserted && lead.auto_wake == Some(false) {
            tx.execute_cached("UPDATE runtime_event_meta SET record=json_set(record,'$.timing.fastSkipReason','autoWake') WHERE id=?1", [format!("complaint:{complaint_id}")]).map_err(studio_runtime_store::StoreError::from)?;
        }
    }
    Ok(typed)
}

/// Python `send_to_user`: the lead submits a complaint to itself, which is
/// addressed to the user, and the record is saved again as an ordinary message.
pub(super) fn submit_user_message(
    tx: &Transaction<'_>,
    lead: &Value,
    request_id: &str,
    text: &str,
    at: f64,
) -> Result<ComplaintRecord, NotificationDispatchError> {
    if lead["id"] != lead["rootId"] {
        return Err(NotificationDispatchError::invalid());
    }
    // TODO(agent-record-chain r3: authorization slice): services/messages.rs::send_as_actor owns this JSON boundary.
    let mut message = submit_agent_complaint(
        tx,
        &AgentRecord::from_value(lead.clone()).map_err(studio_runtime_store::StoreError::from)?,
        request_id,
        text,
        at,
    )?;
    message.source_type = Some(ComplaintSource::Message);
    save_complaint(tx, &message)?;
    Ok(message)
}

/// Transaction-aware implementation of Runtime.complaint read/respond actions.
/// The callback caller owns commit; the response event is durable in this write.
pub(crate) fn agent_complaint_action(
    tx: &Transaction<'_>,
    actor: &AgentRecord,
    args: &Value,
    request_id: &str,
    at: f64,
) -> Result<Value, NotificationDispatchError> {
    let actor_id = actor.id.as_ref();
    let root_id = actor.root_id.as_ref();
    let root = AgentRepository::get(tx, &AgentId::from(root_id))?
        .ok_or_else(|| stored("runtime_agents", root_id))?;
    if root.is_lead != Some(true) || root.deleted_at.is_some() {
        return Err(NotificationDispatchError::invalid());
    }
    match args["action"].as_str() {
        Some("submit") => {
            let text = args["text"]
                .as_str()
                .ok_or(NotificationDispatchError::invalid())?;
            submit_agent_complaint(tx, actor, request_id, text, at).map(ComplaintRecord::into_value)
        }
        Some("read") => {
            if args.get("complaint_id").is_some()
                && !args["complaint_id"].is_null()
                && !args["complaint_id"].is_string()
            {
                return Err(NotificationDispatchError::invalid());
            }
            let requested = args["complaint_id"].as_str();
            let mut complaints = Table::<ComplaintRecord>::new(tx).list()?;
            complaints.retain(|complaint| {
                complaint.lead_id() == Some(root_id)
                    && requested.is_none_or(|id| complaint.id == id)
            });
            if requested.is_some() && complaints.is_empty() {
                return Err(NotificationDispatchError::invalid());
            }
            complaints.sort_by(|left, right| {
                needs_agent_response(right)
                    .cmp(&needs_agent_response(left))
                    .then_with(|| {
                        right
                            .created
                            .as_ref()
                            .and_then(|value| value.seconds())
                            .unwrap_or_default()
                            .total_cmp(
                                &left
                                    .created
                                    .as_ref()
                                    .and_then(|value| value.seconds())
                                    .unwrap_or_default(),
                            )
                    })
            });
            let total = complaints.len();
            for complaint in complaints.iter_mut().take(50) {
                if actor_id == root_id
                    && complaint.recipient == ComplaintRecipient::Lead
                    && complaint.read_at.is_none()
                {
                    complaint.read_at =
                        studio_runtime_schema::complaint_record::ComplaintSeconds::from_seconds(at);
                    save_complaint(tx, complaint)?;
                }
            }
            complaints.truncate(50);
            Ok(json!({
                "complaints":complaints,
                "remaining":total.saturating_sub(50),
                "instruction":"Respond only to entries assigned to the lead. Entries assigned to user wait for the user. Do not poll the book."
            }))
        }
        Some("respond") => {
            if actor_id != root_id {
                return Err(NotificationDispatchError::invalid());
            }
            let id = args["complaint_id"]
                .as_str()
                .ok_or(NotificationDispatchError::invalid())?;
            let raw = studio_runtime_store::QuestionsWriteRepository::complaint(tx, id)?
                .ok_or(NotificationDispatchError::invalid())?;
            let mut complaint: ComplaintRecord =
                studio_json::from_str(&raw).map_err(|_| stored("runtime_complaints", id))?;
            if complaint.id() != id {
                return Err(stored("runtime_complaints", id));
            }
            if complaint.lead_id() != Some(actor_id)
                || complaint.recipient == ComplaintRecipient::User
            {
                return Err(NotificationDispatchError::invalid());
            }
            let text = args["text"]
                .as_str()
                .ok_or(NotificationDispatchError::invalid())?
                .trim();
            let status = studio_runtime_schema::complaints::ComplaintResponseStatus::deserialize(
                &args["status"],
            )
            .map_err(|_| NotificationDispatchError::invalid())?;
            if text.is_empty() || text.chars().count() > 12_000 {
                return Err(NotificationDispatchError::invalid());
            }
            if let Some(existing) = complaint
                .responses
                .iter()
                .find(|response| response.id == request_id)
            {
                if existing.text != text || existing.status != status {
                    return Err(NotificationDispatchError::ConflictingInput);
                }
                return Ok(complaint.into_value());
            }
            let response = studio_runtime_schema::complaint_record::ComplaintResponse {
                id: request_id.to_owned(),
                author: actor_id.to_owned(),
                text: text.to_owned(),
                status,
                at,
            };
            complaint.responses.push(response.clone());
            complaint.status = match status {
                studio_runtime_schema::complaints::ComplaintResponseStatus::InProgress => {
                    ComplaintStatus::InProgress
                }
                studio_runtime_schema::complaints::ComplaintResponseStatus::Resolved => {
                    ComplaintStatus::Resolved
                }
                studio_runtime_schema::complaints::ComplaintResponseStatus::Declined => {
                    ComplaintStatus::Declined
                }
            };
            complaint.updated =
                studio_runtime_schema::complaint_record::ComplaintSeconds::from_seconds(at);
            if complaint.read_at.is_none() {
                complaint.read_at =
                    studio_runtime_schema::complaint_record::ComplaintSeconds::from_seconds(at);
            }
            complaint.version = Some(complaint.version.unwrap_or(0) + 1);
            save_complaint(tx, &complaint)?;
            reconcile_complaint_notifications(tx, actor_id)?;
            let author = complaint.author().unwrap_or_default();
            if author != "user" && author != actor_id {
                let reporter = AgentRepository::get(tx, &AgentId::from(author))?
                    .ok_or_else(|| stored("runtime_agents", author))?;
                if !reporter.is_deleted() {
                    let event = json!({"complaint_id":id,"lead":actor_id,"response":response});
                    crate::event_write::enqueue_event(
                        tx,
                        &reporter,
                        format!("complaint-response:{request_id}"),
                        "complaint_response",
                        event.to_string(),
                        at,
                    )
                    .map_err(|_| stored("runtime_events", id))?;
                }
            }
            Ok(complaint.into_value())
        }
        _ => Err(NotificationDispatchError::invalid()),
    }
}

fn needs_agent_response(complaint: &ComplaintRecord) -> bool {
    if matches!(
        complaint.status,
        ComplaintStatus::Resolved | ComplaintStatus::Declined
    ) {
        return false;
    }
    let responsible = if complaint.recipient == ComplaintRecipient::User {
        "user"
    } else {
        complaint.lead_id().unwrap_or_default()
    };
    !complaint
        .responses
        .iter()
        .any(|response| response.author == responsible)
}

fn reconcile_complaint_notifications(
    tx: &Transaction<'_>,
    lead_id: &str,
) -> Result<(), NotificationDispatchError> {
    let pending = {
        let mut statement = tx.prepare(
            "SELECT id,text FROM runtime_events WHERE agent=?1 AND kind='complaint' AND status='pending'",
        ).map_err(studio_runtime_store::StoreError::from)?;
        statement
            .query_map([lead_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(studio_runtime_store::StoreError::from)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(studio_runtime_store::StoreError::from)?
    };
    for (event_id, text) in pending {
        let Ok(payload) = studio_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(entries) = payload.get("complaints").and_then(Value::as_array) else {
            continue;
        };
        if entries.is_empty() {
            continue;
        }
        let mut all_answered = true;
        let ids: Vec<_> = entries
            .iter()
            .map(|entry| entry.get("complaint_id").and_then(Value::as_str))
            .collect();
        let mut statement = tx
            .prepare_cached(
                "SELECT complaint.record, requested.value FROM json_each(?1) requested \
             LEFT JOIN runtime_complaints complaint ON complaint.id=requested.value",
            )
            .map_err(studio_runtime_store::StoreError::from)?;
        let mut rows = statement
            .query([studio_json::to_string(&ids)?])
            .map_err(studio_runtime_store::StoreError::from)?;
        while let Some(row) = rows
            .next()
            .map_err(studio_runtime_store::StoreError::from)?
        {
            let id: Option<String> = row.get(1).map_err(studio_runtime_store::StoreError::from)?;
            let Some(id) = id else {
                all_answered = false;
                break;
            };
            let raw: Option<String> = row.get(0).map_err(studio_runtime_store::StoreError::from)?;
            let Some(raw) = raw else {
                all_answered = false;
                break;
            };
            let Ok(complaint) = studio_json::from_str::<ComplaintRecord>(&raw) else {
                all_answered = false;
                break;
            };
            if complaint.id() != &id
                || complaint.lead_id() != Some(lead_id)
                || needs_agent_response(&complaint)
            {
                all_answered = false;
                break;
            }
        }
        if all_answered {
            tx.execute_cached(
                "UPDATE runtime_events SET status='stored_only',error=?1 WHERE id=?2 AND status='pending'",
                rusqlite::params!["Every referenced complaint already has a response", event_id],
            ).map_err(studio_runtime_store::StoreError::from)?;
        }
    }
    Ok(())
}

/// Store a complaint row. Its client view is built at read time (`entity_scope`).
fn save_complaint(
    tx: &Transaction<'_>,
    complaint: &ComplaintRecord,
) -> Result<(), NotificationDispatchError> {
    Table::<ComplaintRecord>::new(tx).put(complaint)?;
    Ok(())
}

pub(super) fn completed(
    tx: &Transaction<'_>,
    agent: &AgentRecord,
    params: &Value,
    at: f64,
) -> Result<(), NotificationDispatchError> {
    let questions = fields(&params["item"]["questions"])?;
    let id = crate::questions_reads::async_question_id(
        agent.id.as_ref(),
        params["item"]["id"].as_str().unwrap_or_default(),
    );
    let approval_id = studio_runtime_schema::approvals::ApprovalId(id.clone());
    if studio_runtime_store::approvals::get(tx, &approval_id)
        .map_err(studio_runtime_store::StoreError::from)?
        .is_some()
    {
        return Ok(());
    }
    if agent.is_lead == Some(true) {
        use studio_runtime_schema::approvals as typed;
        let turn = params
            .get("turnId")
            .filter(|v| !v.is_null() && **v != "")
            .map_or(agent.turn_id.as_deref(), Value::as_str)
            .map(|turn| typed::TurnId(turn.to_owned()));
        studio_runtime_store::approvals::create(
            tx,
            studio_runtime_store::approvals::NewApproval {
                id: approval_id,
                agent: typed::AgentId(agent.id.as_ref().to_owned()),
                turn,
                epoch: u64::try_from(agent.epoch).ok(),
                request: typed::Request::AgentQuestion {
                    questions: studio_runtime_store::approvals::questions_from_provider(
                        Value::Array(questions),
                        false,
                    )
                    .map_err(|_| NotificationDispatchError::invalid())?,
                },
                delivery: None,
                detail: typed::Detail {
                    params: typed::ProviderObject(Default::default()),
                    preview: None,
                    answer_error: None,
                    host_close_reason: None,
                },
                fire_at: None,
            },
            typed::Timestamp(at),
        )
        .map_err(studio_runtime_store::StoreError::from)?;
    } else {
        let text = format!(
            "Questions for the orchestrator:\n[{}]",
            questions
                .iter()
                .map(|q| ordered_text(
                    q,
                    ["id", "question", "options", "multiSelect", "isSecret"].map(str::to_owned)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
        submit_agent_complaint(tx, agent, &id, &text, at)?;
    }
    Ok(())
}

pub(super) fn resolved(
    tx: &Transaction<'_>,
    agent: &mut AgentRecord,
    params: &Value,
    account: &str,
    generation: u64,
    at: f64,
) -> Result<(), NotificationDispatchError> {
    use studio_runtime_schema::approvals as typed;
    let id = agent.id.as_ref().to_owned();
    let rows = studio_runtime_store::approvals::for_agent(tx, &typed::AgentId(id))
        .map_err(studio_runtime_store::StoreError::from)?;
    let wanted = match &params["requestId"] {
        Value::Number(number) => number.as_i64().map(|value| typed::RpcId::Number { value }),
        Value::String(value) => Some(typed::RpcId::Text {
            value: value.clone(),
        }),
        Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
    };
    let mut pending = false;
    for mut row in rows {
        // A card names its provider generation: the Rust account worker stores
        // it under the supervisor of its delivery.
        let same_generation = row
            .delivery
            .as_ref()
            .and_then(|d| d.supervisor.as_ref())
            .map(|s| s.generation)
            == Some(generation)
            || row.delivery.as_ref().map(|d| d.connection.as_str())
                == Some(format!("connection-{generation}").as_str());
        let thread = row.detail.params.0.get("threadId");
        if wanted.is_some()
            && row.delivery.as_ref().map(|d| &d.rpc_id) == wanted.as_ref()
            && row.delivery.as_ref().map(|d| d.account.as_str()) == Some(account)
            && same_generation
            && thread == params.get("threadId")
            && matches!(
                row.state,
                typed::ApprovalState::Pending
                    | typed::ApprovalState::Answering { .. }
                    | typed::ApprovalState::Unknown { .. }
            )
        {
            row.state = typed::ApprovalState::Closed {
                reason: typed::CloseReason::ProviderResolved,
                at: typed::Timestamp(at),
            };
            row.fire_at = None;
            studio_runtime_store::approvals::put(tx, &row)
                .map_err(studio_runtime_store::StoreError::from)?;
        }
        pending |= matches!(row.state, typed::ApprovalState::Pending);
    }
    if agent.status == studio_runtime_store::AgentStatus::Approval
        && !pending
        && agent.in_flight == Some(true)
        && agent.auto_wake == Some(true)
    {
        agent.status = studio_runtime_store::AgentStatus::Running;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod reconciliation_tests {
    use super::*;
    use rusqlite::trace::{TraceEvent, TraceEventCodes};

    thread_local! { static READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

    #[test]
    fn reached_blob_preserves_the_lookup_error_column() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_complaints(id TEXT PRIMARY KEY,record TEXT); CREATE TABLE runtime_events(id TEXT PRIMARY KEY,agent TEXT,kind TEXT,text TEXT,status TEXT,error TEXT); INSERT INTO runtime_complaints VALUES('bad',x'ff');").unwrap();
        db.execute(
            "INSERT INTO runtime_events VALUES('event','lead','complaint',?1,'pending',NULL)",
            [
                json!({"complaints":[{"complaint_id":"bad"},{"complaint_id":"absent"}]})
                    .to_string(),
            ],
        )
        .unwrap();
        let tx = db.transaction().unwrap();
        let error = reconcile_complaint_notifications(&tx, "lead").unwrap_err();
        assert!(
            matches!(error, NotificationDispatchError::Store(studio_runtime_store::StoreError::Sqlite(rusqlite::Error::InvalidColumnType(0, ref name, rusqlite::types::Type::Blob))) if name == "record")
        );
    }

    #[test]
    fn reconciles_batched_complaints_and_stops_at_first_missing_entry() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_complaints(id TEXT PRIMARY KEY,record TEXT); CREATE TABLE runtime_events(id TEXT PRIMARY KEY,agent TEXT,kind TEXT,text TEXT,status TEXT,error TEXT);").unwrap();
        let tx = db.transaction().unwrap();
        let mut entries = Vec::new();
        for index in 0..100 {
            let id = format!("c{index}");
            let record = json!({"id":id,"author":"worker","leadId":"lead","text":"question","recipient":"lead","status":"resolved","created":1,"updated":1,"readAt":1,"version":1,"responses":[]});
            tx.execute(
                "INSERT INTO runtime_complaints VALUES(?1,?2)",
                rusqlite::params![id, record.to_string()],
            )
            .unwrap();
            entries.push(json!({"complaint_id":id}));
        }
        tx.execute("INSERT INTO runtime_complaints VALUES('bad',x'ff')", [])
            .unwrap();
        for (id, entries) in [
            ("answered", entries),
            (
                "missing",
                vec![
                    json!({"complaint_id":"absent"}),
                    json!({"complaint_id":"bad"}),
                ],
            ),
        ] {
            tx.execute(
                "INSERT INTO runtime_events VALUES(?1,'lead','complaint',?2,'pending',NULL)",
                rusqlite::params![id, json!({"complaints":entries}).to_string()],
            )
            .unwrap();
        }
        tx.commit().unwrap();
        READS.set(0);
        db.trace_v2(
            TraceEventCodes::SQLITE_TRACE_STMT,
            Some(|event| {
                if let TraceEvent::Stmt(statement, _) = event
                    && statement.sql().contains("LEFT JOIN runtime_complaints")
                {
                    READS.set(READS.get() + 1);
                }
            }),
        );
        let tx = db.transaction().unwrap();
        reconcile_complaint_notifications(&tx, "lead").unwrap();
        let statuses: Vec<String> = ["answered", "missing"]
            .into_iter()
            .map(|id| {
                tx.query_row(
                    "SELECT status FROM runtime_events WHERE id=?1",
                    [id],
                    |row| row.get(0),
                )
                .unwrap()
            })
            .collect();
        assert_eq!(statuses, ["stored_only", "pending"]);
        assert_eq!(READS.get(), 2);
    }
}
