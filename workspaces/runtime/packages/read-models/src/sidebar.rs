//! Sidebar projection policy. Inputs are typed facts from existing domain views.

use crate::chat_status::{ChatStatusFacts, derive_chat_status};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use studio_runtime_schema::agent_entity_nested::AgentStatus;
use studio_runtime_schema::{
    ProviderKind,
    agent_entity_nested_extra::ReadStateDto,
    agent_progress::AgentStartWait,
    project_record::ProjectFolder,
    sidebar::{MarkUnreadEligibility, SidebarChatRow},
};

fn nullable_default<'de, D: serde::Deserializer<'de>, T: Deserialize<'de> + Default>(
    d: D,
) -> Result<T, D::Error> {
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// Typed input from the canonical agent view, rather than a parallel persisted record.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct SidebarFacts {
    pub id: String,
    pub source: Option<studio_runtime_schema::agent_entity_nested::AgentSource>,
    #[serde(default, deserialize_with = "nullable_default")]
    pub is_lead: bool,
    pub name: Option<String>,
    pub project_path: Option<String>,
    pub project: Option<String>,
    pub project_id: Option<String>,
    pub project_server_id: Option<String>,
    pub project_folder: Option<String>,
    #[serde(default, deserialize_with = "nullable_default")]
    pub project_folder_revision: u32,
    pub provider: ProviderKind,
    pub model: Option<String>,
    pub status: Option<AgentStatus>,
    #[serde(default, deserialize_with = "nullable_default")]
    pub epoch: i64,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub stall: Option<studio_runtime_schema::agent_progress::AgentStall>,
    pub last_completed_turn: Option<String>,
    pub last_completed_turn_status: Option<String>,
    pub read_state: Option<ReadStateDto>,
    pub deleted_at: Option<f64>,
    pub shared_room_id: Option<String>,
    pub moved_to: Option<studio_runtime_schema::raw_json::RawJson>,
    #[serde(default, deserialize_with = "nullable_default")]
    pub move_import_pending: bool,
    #[serde(default, deserialize_with = "nullable_default")]
    pub pinned: bool,
    #[serde(default, deserialize_with = "nullable_default")]
    pub archived: bool,
    #[serde(default, deserialize_with = "nullable_default")]
    pub in_flight: bool,
    pub auto_wake: Option<bool>,
    #[serde(default, deserialize_with = "nullable_default")]
    pub empty: bool,
    pub created: Option<f64>,
    #[serde(default)]
    pub updated: studio_runtime_schema::queue_command::Field<f64>,
    pub tail: Option<String>,
    pub start_wait: Option<AgentStartWait>,
    pub capacity_retry:
        Option<studio_runtime_schema::agent_entity_nested_extra::AgentEntityCapacityRetryDto>,
    pub account_key: Option<String>,
    pub parent_id: Option<String>,
    pub parked_event: Option<String>,
    #[serde(default, deserialize_with = "nullable_default")]
    pub has_question: bool,
    #[serde(default, deserialize_with = "nullable_default")]
    pub has_approval: bool,
    #[serde(default, deserialize_with = "nullable_default")]
    pub has_background_work: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SidebarContext {
    pub team_id: Option<String>,
    pub team_name: String,
    pub team_same_project: bool,
    pub project_name: String,
    pub folders: Vec<ProjectFolder>,
    pub order: BTreeMap<String, Vec<String>>,
    pub wait_label: Option<String>,
    pub answer: bool,
    pub active: bool,
    pub deferred: bool,
}

pub fn visible(facts: &SidebarFacts) -> bool {
    facts.source == Some(studio_runtime_schema::agent_entity_nested::AgentSource::Managed)
        && facts.is_lead
        && facts.deleted_at.is_none()
        && facts.shared_room_id.as_deref().is_none_or(str::is_empty)
        && facts.moved_to.is_none()
        && !facts.move_import_pending
}

pub fn project(facts: &SidebarFacts, context: &SidebarContext) -> Option<SidebarChatRow> {
    if !visible(facts) {
        return None;
    }
    let path = facts.project_path.clone().unwrap_or_default();
    let folder = facts.project_folder.clone().filter(|s| !s.is_empty());
    let mut legacy = vec![
        studio_json::json!("chats"),
        studio_json::json!(facts.project_path),
        studio_json::json!(facts.pinned),
        studio_json::json!(facts.archived),
    ];
    if let Some(team) = &context.team_id {
        legacy.extend([studio_json::json!("team"), studio_json::json!(team)]);
    }
    if let Some(folder) = &folder {
        legacy.push(studio_json::json!(folder));
    }
    let legacy = studio_json::to_string(&legacy).expect("string tuple serializes");
    let group_key = if context.team_id.is_none() && !facts.pinned {
        studio_json::json!(["items", path, folder]).to_string()
    } else {
        legacy.clone()
    };
    let rank = |key: &str| {
        context
            .order
            .get(key)?
            .iter()
            .position(|id| *id == facts.id)
            .and_then(|i| u32::try_from(i).ok())
    };
    let order_rank = rank(&group_key).or_else(|| rank(&legacy));
    let mark_read = crate::chat_status::mark_read_target(
        facts.thread_id.as_deref(),
        facts.last_completed_turn.as_deref(),
        facts.last_completed_turn_status.as_deref(),
        facts.read_state.as_ref(),
    );
    let unread = mark_read.as_ref().is_some_and(|r| !r.read);
    let indicator = derive_chat_status(&ChatStatusFacts {
        status: facts.status.as_ref(), auto_wake: facts.auto_wake, empty: facts.empty, in_flight: facts.in_flight,
        answer: context.answer || facts.has_question || facts.has_approval,
        deferred: context.deferred, background_work: facts.has_background_work,
        active: context.active || facts.has_background_work || facts.in_flight || (facts.auto_wake != Some(false) && matches!(facts.status, Some(AgentStatus::Queued | AgentStatus::Starting | AgentStatus::Running))),
        disk_full: facts.capacity_retry.as_ref().is_some_and(|retry| {
            retry.thread_id == facts.thread_id && retry.epoch.as_ref().map(|e| e.as_i64()) == Some(facts.epoch)
                && retry.account_key.as_deref().unwrap_or("default") == facts.account_key.as_deref().unwrap_or("default")
                && retry.status == "scheduled" && retry.accepted_turn_id.as_ref().is_none_or(String::is_empty)
                && retry.cause.as_ref() == Some(&studio_runtime_schema::continuation_snapshot::CapacityContinuationCause::DiskFull)
        }), unread, wait_label: context.wait_label.as_deref(),
    });
    let activity_at = match facts.updated {
        studio_runtime_schema::queue_command::Field::Missing => facts.created.unwrap_or_default(),
        studio_runtime_schema::queue_command::Field::Null => 0.0,
        studio_runtime_schema::queue_command::Field::Value(value) => value,
    };
    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    let mut next = folder.as_deref();
    while let Some(id) = next {
        if !seen.insert(id) {
            break;
        }
        let Some(item) = context.folders.iter().find(|f| f.id == id) else {
            break;
        };
        names.push(item.name.as_str());
        next = item.parent_id.as_ref().map(String::as_str);
    }
    names.reverse();
    let name = facts.name.clone().unwrap_or_default();
    let search_text = format!(
        "{} {} {} {} {} {}",
        if context.team_same_project {
            &context.team_name
        } else {
            ""
        },
        name,
        path,
        if context.project_name.is_empty() {
            facts.project.as_deref().unwrap_or_default()
        } else {
            &context.project_name
        },
        names.join(" / "),
        facts.tail.as_deref().unwrap_or_default()
    )
    .to_lowercase();
    let copy_progress = match &facts.start_wait {
        Some(AgentStartWait::WorkspaceCopy { epoch, progress })
            if *epoch == facts.epoch
                && matches!(
                    facts.status,
                    Some(AgentStatus::Queued | AgentStatus::Starting)
                ) =>
        {
            Some(progress.clone())
        }
        _ => None,
    };
    Some(SidebarChatRow {
        id: facts.id.clone(),
        name,
        project_path: path,
        project_id: facts.project_id.clone(),
        project_server_id: facts.project_server_id.clone(),
        project_folder: folder,
        project_folder_revision: facts.project_folder_revision,
        team_id: context.team_id.clone(),
        group_key,
        sort_key: sort_key(facts.pinned, order_rank, activity_at, &facts.id),
        order_rank,
        activity_at,
        pinned: facts.pinned,
        archived: facts.archived,
        in_flight: facts.in_flight,
        provider: facts.provider,
        model: facts.model.clone(),
        indicator,
        mark_unread: if mark_read.is_none() {
            MarkUnreadEligibility::NoCompletedResult
        } else if unread {
            MarkUnreadEligibility::AlreadyUnread
        } else {
            MarkUnreadEligibility::Allowed
        },
        mark_read,
        can_change_project: facts.thread_id.as_deref().is_none_or(str::is_empty)
            && !facts.in_flight,
        can_archive: !facts.in_flight,
        compact_pinned: context.team_same_project
            || facts.pinned
            || facts.in_flight
            || unread
            || matches!(
                facts.status,
                Some(AgentStatus::Running | AgentStatus::Starting | AgentStatus::Approval)
            ),
        search_text,
        copy_progress,
        waiting_for_lead: facts.stall.is_some(),
    })
}

/// ASCII lexicographic key: pinned first (0/1), ranked first (0/1), u32 rank
/// as 8 hex digits, reverse IEEE-754 activity order as 16 hex digits, then id.
/// Flip the positive sign bit / invert negative bits to encode numeric order;
/// invert that result for newest first. Normalize signed zero. Fixed-width
/// fields make the complete key comparable without parsing or client policy.
fn sort_key(pinned: bool, rank: Option<u32>, activity: f64, id: &str) -> String {
    let bits = (if activity == 0.0 { 0.0 } else { activity }).to_bits();
    let ascending = if bits >> 63 == 0 {
        bits ^ (1_u64 << 63)
    } else {
        !bits
    };
    format!(
        "{}{}{:08x}{:016x}{}",
        u8::from(!pinned),
        u8::from(rank.is_none()),
        rank.unwrap_or_default(),
        !ascending,
        id
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_json::json;

    #[test]
    fn sidebar_projector_owns_unread_status_order_and_copy_epoch() {
        let source: SidebarFacts = studio_json::from_value(json!({
            "id":"chat","source":"managed","isLead":true,"name":"Title","projectPath":"/p",
            "provider":"codex","status":"completed","threadId":"t","lastCompletedTurn":"r",
            "lastCompletedTurnStatus":"completed","created":100,"updated":200,"epoch":2,
            "startWait":{"reason":"workspace_copy","epoch":2,"progress":{"phase":"staging","done":1,"unit":"files"}}
        })).expect("typed facts");
        let context = SidebarContext::default();
        let row = project(&source, &context).expect("visible lead");
        assert_eq!(
            row.indicator.kind,
            studio_runtime_schema::chat_operational_status::ChatOperationalKind::Unread
        );
        assert_eq!(
            row.mark_read.as_ref().expect("CAS proof").revision.as_i64(),
            0
        );
        assert!(
            row.copy_progress.is_none(),
            "completed status hides old copy progress"
        );
        assert!(row.compact_pinned, "unread results survive compact mode");
        assert_eq!(row.search_text, " title /p   ");
        let mut read = source.clone();
        read.read_state = Some(
            studio_json::from_value(json!({"threadId":"t","turnId":"r","read":true,"revision":3}))
                .expect("receipt"),
        );
        let read_row = project(&read, &context).expect("read row");
        assert_eq!(
            read_row.indicator.kind,
            studio_runtime_schema::chat_operational_status::ChatOperationalKind::None
        );
        assert_eq!(
            read_row.mark_unread,
            studio_runtime_schema::sidebar::MarkUnreadEligibility::Allowed
        );
        let mut pinned = read.clone();
        pinned.id = "pinned".into();
        pinned.pinned = true;
        let pinned = project(&pinned, &context).expect("pinned");
        assert!(pinned.sort_key < read_row.sort_key);
        let mut copying = read.clone();
        copying.status = Some(studio_runtime_schema::agent_entity_nested::AgentStatus::Starting);
        assert!(
            project(&copying, &context)
                .expect("copy")
                .copy_progress
                .is_some()
        );
        copying.epoch = 3;
        assert!(
            project(&copying, &context)
                .expect("stale copy")
                .copy_progress
                .is_none()
        );
    }

    #[test]
    fn sort_key_orders_rank_then_activity_then_id_and_visibility_is_exact() {
        let source: SidebarFacts = studio_json::from_value(json!({"id":"b","source":"managed","isLead":true,"provider":"codex","status":"idle","updated":1.5})).expect("facts");
        let mut context = SidebarContext::default();
        let unranked = project(&source, &context).expect("row");
        context
            .order
            .insert("[\"items\",\"\",null]".into(), vec!["b".into()]);
        let ranked = project(&source, &context).expect("ranked");
        assert!(ranked.sort_key < unranked.sort_key);
        let mut newer = source.clone();
        newer.updated = studio_runtime_schema::queue_command::Field::Value(2.0);
        assert!(
            project(&newer, &SidebarContext::default())
                .expect("newer")
                .sort_key
                < unranked.sort_key
        );
        for field in ["deletedAt", "sharedRoomId", "movedTo", "moveImportPending"] {
            let mut value = json!({"id":"b","source":"managed","isLead":true,"provider":"codex"});
            value[field] = match field {
                "deletedAt" => json!(1),
                "moveImportPending" => json!(true),
                _ => json!("other"),
            };
            assert!(project(&studio_json::from_value(value).expect("facts"), &context).is_none());
        }
    }
}

/// A live task or monitor classified by the existing process display owner.
pub struct WaitActivity {
    pub owner: String,
    pub command: bool,
    pub monitor: bool,
    pub background: bool,
}

/// Apply the sidebar's operational context from checked, current-epoch facts.
/// The recursive child predicate matches the visible wait reasons, so the label
/// and spinner have the same source. This function performs no reads.
pub fn status_context(
    context: &mut SidebarContext,
    facts: &SidebarFacts,
    members: &[SidebarFacts],
    activities: &[WaitActivity],
    inputs: &BTreeMap<String, u32>,
    deferred: &BTreeSet<String>,
) {
    context.answer = members.iter().any(|m| m.has_question || m.has_approval);
    context.active = !activities.is_empty()
        || members.iter().any(|m| {
            m.has_background_work
                || m.in_flight
                || (m.auto_wake != Some(false)
                    && matches!(
                        m.status,
                        Some(AgentStatus::Queued | AgentStatus::Starting | AgentStatus::Running)
                    ))
        });
    context.deferred = deferred.contains(&facts.id);
    let has_work = |child: &SidebarFacts| {
        let mut pending = vec![child];
        let mut seen = BTreeSet::from([facts.id.as_str()]);
        while let Some(current) = pending.pop() {
            if !seen.insert(current.id.as_str())
                || current.archived
                || matches!(
                    current.status,
                    Some(AgentStatus::Paused | AgentStatus::Failed | AgentStatus::Interrupted)
                )
            {
                continue;
            }
            if current.in_flight
                || matches!(
                    current.status,
                    Some(
                        AgentStatus::Queued
                            | AgentStatus::Starting
                            | AgentStatus::Running
                            | AgentStatus::Approval
                    )
                )
            {
                return true;
            }
            let parked = current.status == Some(AgentStatus::Parked)
                && current.parked_event.as_ref().is_some_and(|s| !s.is_empty());
            if (current.auto_wake == Some(false) && !parked)
                || !matches!(
                    current.status,
                    Some(
                        AgentStatus::Waiting | AgentStatus::Parked | AgentStatus::WaitingOnMonitor
                    )
                )
            {
                continue;
            }
            if parked
                || activities
                    .iter()
                    .any(|a| a.owner == current.id && (a.monitor || a.command))
            {
                return true;
            }
            pending.extend(
                members
                    .iter()
                    .filter(|m| m.parent_id.as_deref() == Some(&current.id)),
            );
        }
        false
    };
    let children = members
        .iter()
        .filter(|m| m.parent_id.as_deref() == Some(&facts.id) && has_work(m))
        .count();
    let commands = activities
        .iter()
        .filter(|a| a.owner == facts.id && a.command && (a.background || !facts.in_flight))
        .count();
    let monitors = activities
        .iter()
        .filter(|a| a.owner == facts.id && a.monitor)
        .count();
    let mut reasons = Vec::new();
    for (count, name) in [
        (children, "agent"),
        (commands, "command"),
        (monitors, "monitor"),
    ] {
        if count > 0 {
            reasons.push(format!(
                "{count} {name}{}",
                if count == 1 { "" } else { "s" }
            ));
        }
    }
    if let Some(event) = facts.parked_event.as_ref().filter(|s| !s.is_empty()) {
        reasons.push(format!("event {event}"));
    }
    let inputs = inputs.get(&facts.id).copied().unwrap_or_default();
    if inputs > 0 {
        reasons.push(format!(
            "{inputs} input{}",
            if inputs == 1 { "" } else { "s" }
        ));
    }
    context.wait_label = match reasons.split_last() {
        None => None,
        Some((last, [])) => Some(format!("Waiting for {last}")),
        Some((last, rest)) => Some(format!("Waiting for {} and {last}", rest.join(", "))),
    };
}

#[cfg(test)]
mod wait_tests {
    use super::*;
    use studio_json::json;
    #[test]
    fn wait_spinner_uses_current_children_commands_inputs_and_event() {
        let lead: SidebarFacts = studio_json::from_value(
            json!({"id":"lead","provider":"codex","status":"waiting","parkedEvent":"ready"}),
        )
        .expect("lead");
        let worker: SidebarFacts = studio_json::from_value(json!({"id":"worker","provider":"codex","parentId":"lead","status":"running","autoWake":true})).expect("worker");
        let mut context = SidebarContext::default();
        status_context(
            &mut context,
            &lead,
            &[lead.clone(), worker],
            &[WaitActivity {
                owner: "lead".into(),
                command: true,
                monitor: false,
                background: true,
            }],
            &BTreeMap::from([("lead".into(), 1)]),
            &BTreeSet::new(),
        );
        assert_eq!(
            context.wait_label.as_deref(),
            Some("Waiting for 1 agent, 1 command, event ready and 1 input")
        );
        assert!(context.active);
    }
}
