//! Wake candidates come from sparse indexes; the scheduler decides eligibility in Rust.
// Limitation: duplicate selector keys are unsupported; the 2026-11-12 live audit found none.
use super::*;
use crate::agent_tables::{AgentsStorageRow, ChatsStorageRow, TurnsStorageRow};
use crate::peer_update_settings::{Canvas, canvas_read};
use crate::queue_schema::EventsRow;
use studio_db_api::DbQuery;
use studio_runtime_schema::persistence::{EventStatus, HoldStatus};

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=String,
    tables(rows=EventsRow),columns(agent=EventsRow::AGENT,status=EventsRow::STATUS),
    sql="SELECT {agent} FROM {rows} WHERE {status}=?1 ORDER BY {agent}")]
struct PendingOwners {
    #[db(column=EventsRow::STATUS)]
    status: EventStatus,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=String,
    tables(rows=ChatsStorageRow),indexes(rows=ChatsStorageRow::INDEX_CHATS_WAKE_QUEUED),predicates(queued=rows),
    columns(id=ChatsStorageRow::ID),sql="SELECT {id} FROM {rows} WHERE {queued} AND {id}>=?1 ORDER BY {id}")]
struct QueuedOwners<'a> {
    #[db(column=ChatsStorageRow::ID)]
    after: &'a str,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=String,
    tables(rows=AgentsStorageRow),indexes(rows=AgentsStorageRow::INDEX_RUNTIME_AGENT_WAKE_RECOVERY),predicates(recovery=rows),
    columns(id=AgentsStorageRow::ID),sql="SELECT {id} FROM {rows} WHERE ({recovery}) AND {id}>=?1 ORDER BY {id}")]
struct RecoveryOwners<'a> {
    #[db(column=AgentsStorageRow::ID)]
    after: &'a str,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=String,
    tables(rows=TurnsStorageRow),indexes(rows=TurnsStorageRow::INDEX_TURNS_WAKE_SUBMITTED),predicates(submitted=rows),
    columns(chat=TurnsStorageRow::CHAT),sql="SELECT {chat} FROM {rows} WHERE {submitted} AND {chat}>=?1 ORDER BY {chat}")]
struct SubmittedOwners<'a> {
    #[db(column=TurnsStorageRow::CHAT)]
    after: &'a str,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=optional,native,result=String,
    tables(rows=AgentsStorageRow),columns(record=AgentsStorageRow::RECORD,id=AgentsStorageRow::ID,hold=AgentsStorageRow::HOLD_STATUS),
    sql="SELECT {record} FROM {rows} WHERE {id}=?1 AND {hold}=?2")]
struct AgentSource<'a> {
    #[db(column=AgentsStorageRow::ID)]
    id: &'a str,
    #[db(column=AgentsStorageRow::HOLD_STATUS)]
    hold: HoldStatus,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=optional,native,result=(String,Option<i64>,Option<String>),
    tables(rows=ChatsStorageRow),columns(id=ChatsStorageRow::ID,status=ChatsStorageRow::STATUS,auto=ChatsStorageRow::AUTO_WAKE,mode=ChatsStorageRow::EXECUTION_MODE),
    sql="SELECT {status},{auto},{mode} FROM {rows} WHERE {id}=?1")]
struct Chat<'a> {
    #[db(column=ChatsStorageRow::ID)]
    id: &'a str,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,Option<String>,Option<i64>,Option<i64>),
    tables(rows=TurnsStorageRow),columns(chat=TurnsStorageRow::CHAT,state=TurnsStorageRow::STATE,provider=TurnsStorageRow::PROVIDER_TURN,submitted=TurnsStorageRow::ATTEMPT_SUBMITTED,generation=TurnsStorageRow::ATTEMPT_CONNECTION_GENERATION),
    sql="SELECT {state},{provider},{submitted},{generation} FROM {rows} WHERE {chat}=?1")]
struct Turns<'a> {
    #[db(column=TurnsStorageRow::CHAT)]
    chat: &'a str,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,f64,i64),
    tables(rows=EventsRow),columns(agent=EventsRow::AGENT,epoch=EventsRow::EPOCH,status=EventsRow::STATUS,kind=EventsRow::KIND,created=EventsRow::CREATED),
    sql="SELECT {kind},{created},{epoch} FROM {rows} WHERE {agent}=?1 AND {status}=?2 ORDER BY {created}")]
struct Events<'a> {
    #[db(column=EventsRow::AGENT)]
    agent: &'a str,
    #[db(column=EventsRow::STATUS)]
    status: EventStatus,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=String,
    tables(rows=AgentsStorageRow),columns(id=AgentsStorageRow::ID,parent=AgentsStorageRow::PARENT_ID),
    sql="SELECT {id} FROM {rows} WHERE {parent}=?1")]
struct Children<'a> {
    #[db(column=AgentsStorageRow::PARENT_ID)]
    parent: Option<&'a str>,
}

/// Keep projection lazy: invalid epoch/mode on an unselected account was never decoded.
pub(super) type Sources = BTreeMap<String, Vec<(String, Value)>>;

fn zero(value: &Value) -> bool {
    value.is_null() || value == false || value.as_f64() == Some(0.0)
}

// These comparisons retain SQLite's INTEGER event affinity and TEXT selector affinity.
fn same_epoch(value: &Value, epoch: i64) -> bool {
    match value {
        Value::Bool(value) => epoch == i64::from(*value),
        Value::Number(value) => value.as_i64().map_or_else(
            || {
                value
                    .as_f64()
                    .is_some_and(|value| same_real_epoch(value, epoch))
            },
            |value| value == epoch,
        ),
        Value::String(value) => value.trim().parse::<i64>().map_or_else(
            |_| {
                value
                    .trim()
                    .parse::<f64>()
                    .is_ok_and(|value| same_real_epoch(value, epoch))
            },
            |value| value == epoch,
        ),
        Value::Null | Value::Array(_) | Value::Object(_) => false,
    }
}
fn same_real_epoch(value: f64, epoch: i64) -> bool {
    value.is_finite()
        && value.fract() == 0.0
        && value >= i64::MIN as f64
        && value < -(i64::MIN as f64)
        && value as i64 == epoch
}
fn selector_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(value) => Some(value.clone()),
        Value::Bool(value) => Some(i64::from(*value).to_string()),
        value @ (Value::Number(_) | Value::Array(_) | Value::Object(_)) => Some(value.to_string()),
    }
}

fn live(record: &Value) -> bool {
    record["deletedAt"].is_null() && zero(&record["remoteWorker"]) && zero(&record["remoteAnchor"])
}

fn has_active_child(db: &Connection, parent: &str) -> StoreResult<bool> {
    let read = canvas_read(db);
    for id in read.collect(
        &Children::QUERY,
        &Children {
            parent: Some(parent),
        },
    )? {
        let Some(raw) = studio_db_api::db_query!(
            read,
            AgentsStorageRow,
            scalar_raw(AgentsStorageRow::RECORD),
            filter(AgentsStorageRow::ID.eq_ref(id.as_str()))
        )?
        else {
            continue;
        };
        let record: Value = studio_json::from_str(&raw)?;
        if !record["deletedAt"].is_null() {
            continue;
        }
        if read
            .named(&Chat::QUERY, &Chat { id: &id })?
            .is_some_and(|chat| matches!(chat.0.as_str(), "queued" | "starting" | "running"))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn sources(db: &Connection, held_after: f64) -> StoreResult<Sources> {
    let read = canvas_read(db);
    let mut ids: BTreeSet<String> = read
        .collect(
            &PendingOwners::QUERY,
            &PendingOwners {
                status: EventStatus::Pending,
            },
        )?
        .into_iter()
        .collect();
    ids.extend(read.collect(&QueuedOwners::QUERY, &QueuedOwners { after: "" })?);
    ids.extend(read.collect(&RecoveryOwners::QUERY, &RecoveryOwners { after: "" })?);
    ids.extend(read.collect(&SubmittedOwners::QUERY, &SubmittedOwners { after: "" })?);
    let mut result = Sources::new();
    for id in ids {
        let Some(raw) = read.named(
            &AgentSource::QUERY,
            &AgentSource {
                id: &id,
                hold: HoldStatus::Clear,
            },
        )?
        else {
            continue;
        };
        let record: Value = studio_json::from_str(&raw)?;
        if !live(&record) {
            continue;
        }
        let chat = read.named(&Chat::QUERY, &Chat { id: &id })?;
        let status = chat.as_ref().map_or("", |chat| chat.0.as_str());
        let turns = read.collect(&Turns::QUERY, &Turns { chat: &id })?;
        let unfinished = turns
            .iter()
            .find(|turn| crate::domain::TurnState::UNFINISHED.contains(&turn.0.as_str()));
        let queued = status == "queued"
            && chat.as_ref().is_some_and(|chat| chat.1 == Some(1))
            && unfinished.is_none()
            && zero(&record["nativeFailureHold"]);
        let steering = status == "running"
            && chat
                .as_ref()
                .is_some_and(|chat| chat.1 == Some(1) && chat.2.as_deref() != Some("vm"))
            && zero(&record["nativeFailureHold"])
            && unfinished
                .and_then(|turn| turn.1.as_deref())
                .is_some_and(|turn| {
                    selector_text(&record["steerRejectedTurnId"])
                        .as_deref()
                        .unwrap_or("")
                        != turn
                });
        let mut pending = queued && record["nativeReview"]["status"] == "pending";
        if queued || steering {
            for (kind, created, event_epoch) in read.collect(
                &Events::QUERY,
                &Events {
                    agent: &id,
                    status: EventStatus::Pending,
                },
            )? {
                if !same_epoch(&record["epoch"], event_epoch) {
                    continue;
                }
                if steering && !DEDICATED_TURN_EVENT_KINDS.contains(&kind.as_str()) {
                    pending = true;
                    break;
                }
                if queued
                    && !(kind == "child_result"
                        && created > held_after
                        && has_active_child(db, &id)?)
                {
                    pending = true;
                    break;
                }
            }
        }
        let settled = matches!(status, "completed" | "waiting" | "paused" | "parked");
        let recovery = (record["provider"] == "codex"
            && matches!(
                status,
                "starting" | "running" | "waiting" | "paused" | "interrupted"
            )
            && turns
                .iter()
                .any(|turn| turn.2 == Some(1) && turn.3.is_some()))
            || (record["disconnectRecovery"].is_object()
                && (!settled || !record["disconnectRecovery"]["stage"].is_null()))
            || (record["restartRecovery"].is_object()
                && (!settled
                    || !matches!(
                        record["restartRecovery"]["stage"].as_str(),
                        Some("finished" | "continued" | "reattached" | "input_restored")
                    )));
        if !pending && !recovery {
            continue;
        }
        let account = match record.get("accountKey") {
            None | Some(Value::Null) => "default",
            Some(Value::String(account)) if !account.is_empty() => account,
            _ => {
                return Err(StoreError::InvalidStoredObject {
                    table: "runtime_agents",
                    id,
                });
            }
        };
        let entries = result.entry(account.to_owned()).or_default();
        if pending {
            entries.push((id, record));
        }
    }
    Ok(result)
}

pub(super) fn candidates(
    db: &Connection,
    rows: &[(String, Value)],
    after: &str,
    limit: usize,
) -> StoreResult<Vec<WakeCandidate>> {
    rows.iter()
        .filter(|(id, _)| id.as_str() > after)
        .take(limit)
        .map(|(id, record)| {
            let epoch = match &record["epoch"] {
                Value::Bool(value) => Some(i64::from(*value)),
                Value::Number(value) => value.as_i64(),
                Value::Null | Value::String(_) | Value::Array(_) | Value::Object(_) => None,
            }
            .ok_or_else(|| {
                let ty = match &record["epoch"] {
                    Value::Null => rusqlite::types::Type::Null,
                    Value::Number(_) => rusqlite::types::Type::Real,
                    Value::Bool(_) | Value::String(_) | Value::Array(_) | Value::Object(_) => {
                        rusqlite::types::Type::Text
                    }
                };
                rusqlite::Error::InvalidColumnType(1, "json_extract(a.record,'$.epoch')".into(), ty)
            })?;
            let workspace_mode = <Option<WorkspaceMode> as serde::Deserialize>::deserialize(
                record["workspaceMode"].clone(),
            )
            .map_err(|_| {
                std::sync::Arc::new(
                    match AgentRepository::get(db, &AgentId::from(id.as_str())) {
                        Err(error) => error,
                        Ok(_) => StoreError::RecordContract(
                            "workspaceMode projection disagrees with its generated owner",
                        ),
                    },
                )
            });
            Ok(WakeCandidate {
                agent_id: id.clone(),
                epoch,
                workspace_mode,
            })
        })
        .collect()
}

/// The existing scheduler index owner installs exactly the descriptor-owned sparse indexes.
pub(super) fn prepare_indexes(tx: &Transaction<'_>) -> StoreResult<()> {
    crate::queue_schema::install_index::<AgentsStorageRow>(
        tx,
        studio_db_api::query_index::<Canvas, AgentsStorageRow>(
            &AgentsStorageRow::INDEX_RUNTIME_AGENT_WAKE_RECOVERY,
        ),
    )?;
    crate::queue_schema::install_index::<ChatsStorageRow>(
        tx,
        studio_db_api::query_index::<Canvas, ChatsStorageRow>(
            &ChatsStorageRow::INDEX_CHATS_WAKE_QUEUED,
        ),
    )?;
    crate::queue_schema::install_index::<TurnsStorageRow>(
        tx,
        studio_db_api::query_index::<Canvas, TurnsStorageRow>(
            &TurnsStorageRow::INDEX_TURNS_WAKE_SUBMITTED,
        ),
    )?;
    Ok(())
}

#[cfg(test)]
pub(super) fn assert_candidate_plans(db: &Connection) {
    use crate::query_plan_test_support::explain;
    let plans = [
        explain(
            db,
            PendingOwners::QUERY.sql(),
            &PendingOwners {
                status: EventStatus::Pending,
            },
        )
        .unwrap(),
        explain(db, QueuedOwners::QUERY.sql(), &QueuedOwners { after: "" }).unwrap(),
        explain(
            db,
            RecoveryOwners::QUERY.sql(),
            &RecoveryOwners { after: "" },
        )
        .unwrap(),
        explain(
            db,
            SubmittedOwners::QUERY.sql(),
            &SubmittedOwners { after: "" },
        )
        .unwrap(),
    ];
    eprintln!("wake candidate plans: {plans:?}");
    assert!(
        plans.iter().flatten().all(|step| !step.contains("SCAN ")
            && !step.contains("CORRELATED")
            && !step.contains("TEMP B-TREE")),
        "{plans:?}"
    );
}
