//! Domain operational precedence, shared by summaries and system read proofs.
use studio_runtime_schema::agent_entity_nested::AgentStatus;
use studio_runtime_schema::chat_operational_status::{
    AgentOperationalStatus as O, AgentWaitReasons,
};
#[allow(clippy::struct_excessive_bools)]
pub struct ChatStatusFacts<'a> {
    pub status: Option<&'a AgentStatus>,
    pub auto_wake: Option<bool>,
    pub empty: bool,
    pub in_flight: bool,
    pub answer: bool,
    pub deferred: bool,
    pub background_work: bool,
    pub active: bool,
    pub disk_full: bool,
    pub unread: bool,
    pub wait: Option<&'a AgentWaitReasons>,
}
pub fn derive_chat_status(f: &ChatStatusFacts<'_>) -> O {
    use AgentStatus as S;
    let waiting = matches!(f.status, Some(S::Waiting | S::Parked | S::WaitingOnMonitor));
    if f.answer {
        return O::NeedsInput {
            wait: if waiting { f.wait.cloned() } else { None },
        };
    }
    if waiting && f.background_work {
        return O::WaitingBackgroundCommand {};
    }
    if waiting {
        return f
            .wait
            .cloned()
            .map_or(O::TurnEnded {}, |wait| O::Waiting { wait });
    }
    if f.active {
        return if !f.in_flight {
            f.wait
                .cloned()
                .map_or(O::Working {}, |wait| O::Waiting { wait })
        } else {
            O::Working {}
        };
    }
    if matches!(f.status, Some(S::Failed)) {
        return if f.disk_full {
            O::DiskSpace {}
        } else {
            O::Failed {}
        };
    }
    if matches!(f.status, Some(S::Paused | S::Interrupted))
        || (f.auto_wake == Some(false) && !f.empty)
    {
        return if matches!(f.status, Some(S::Interrupted)) {
            O::Interrupted {}
        } else {
            O::Stopped {}
        };
    }
    if f.deferred && matches!(f.status, Some(S::Approval)) {
        return O::QuestionDeferred {};
    }
    if matches!(f.status, Some(S::Completed)) {
        return if f.unread {
            O::UnreadResult {}
        } else {
            O::ReadResult {}
        };
    }
    O::Ready {}
}

/// Completed-result receipt projection shared by sidebar rows and the system summary.
/// The revision comes from the saved receipt even when it names an older result,
/// preserving the existing compare-and-set proof for an explicit action.
pub fn mark_read_target(
    thread: Option<&str>,
    completed_turn: Option<&str>,
    completed_status: Option<&str>,
    receipt: Option<&studio_runtime_schema::agent_entity_nested_extra::ReadStateDto>,
) -> Option<studio_runtime_schema::agent_entity_nested_extra::ReadStateDto> {
    let (thread, turn) = thread
        .filter(|s| !s.is_empty())
        .zip(completed_turn.filter(|s| !s.is_empty()))?;
    if completed_status != Some("completed") {
        return None;
    }
    Some(
        studio_runtime_schema::agent_entity_nested_extra::ReadStateDto {
            thread_id: thread.to_owned(),
            turn_id: turn.to_owned(),
            read: receipt.is_some_and(|r| r.read && r.thread_id == thread && r.turn_id == turn),
            revision: receipt.map_or_else(
                || studio_runtime_schema::AgentCounter::from_i64(0).expect("zero counter"),
                |r| r.revision.clone(),
            ),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_runtime_schema::chat_operational_status::WaitReasonFields;
    #[test]
    fn v5_active_wait_and_input_precedence_use_the_same_checked_reasons() {
        let wait = AgentWaitReasons::new(WaitReasonFields {
            agents: 1,
            commands: 0,
            monitors: 0,
            inputs: 0,
            event: None,
        })
        .expect("live wait");
        let mut f = ChatStatusFacts {
            status: Some(&AgentStatus::Running),
            auto_wake: Some(true),
            empty: false,
            in_flight: false,
            answer: false,
            deferred: false,
            background_work: false,
            active: true,
            disk_full: false,
            unread: false,
            wait: Some(&wait),
        };
        assert_eq!(derive_chat_status(&f), O::Waiting { wait: wait.clone() });
        f.in_flight = true;
        assert_eq!(derive_chat_status(&f), O::Working {});
        f.status = Some(&AgentStatus::Waiting);
        f.answer = true;
        assert_eq!(
            derive_chat_status(&f),
            O::NeedsInput {
                wait: Some(wait.clone())
            }
        );
        f.answer = false;
        f.background_work = true;
        assert_eq!(derive_chat_status(&f), O::WaitingBackgroundCommand {});
    }
    #[test]
    fn v5_persisted_unread_does_not_override_a_stopped_or_failed_turn() {
        let mut f = ChatStatusFacts {
            status: Some(&AgentStatus::Completed),
            auto_wake: Some(false),
            empty: false,
            in_flight: false,
            answer: false,
            deferred: false,
            background_work: false,
            active: false,
            disk_full: false,
            unread: true,
            wait: None,
        };
        assert_eq!(derive_chat_status(&f), O::Stopped {});
        f.auto_wake = Some(true);
        assert_eq!(derive_chat_status(&f), O::UnreadResult {});
        f.status = Some(&AgentStatus::Failed);
        f.disk_full = true;
        assert_eq!(derive_chat_status(&f), O::DiskSpace {});
        f.disk_full = false;
        assert_eq!(derive_chat_status(&f), O::Failed {});
    }
}

#[cfg(test)]
mod receipt_and_deferral_tests {
    use super::*;
    #[test]
    fn v5_deferral_and_empty_stop_rules_do_not_mask_current_attention() {
        let mut f = ChatStatusFacts {
            status: Some(&AgentStatus::Approval),
            auto_wake: Some(true),
            empty: false,
            in_flight: false,
            answer: false,
            deferred: true,
            background_work: false,
            active: false,
            disk_full: false,
            unread: false,
            wait: None,
        };
        assert_eq!(derive_chat_status(&f), O::QuestionDeferred {});
        f.answer = true;
        assert_eq!(derive_chat_status(&f), O::NeedsInput { wait: None });
        f.answer = false;
        f.status = Some(&AgentStatus::Interrupted);
        assert_eq!(derive_chat_status(&f), O::Interrupted {});
        f.status = Some(&AgentStatus::Idle);
        f.auto_wake = Some(false);
        assert_eq!(derive_chat_status(&f), O::Stopped {});
        f.empty = true;
        assert_eq!(derive_chat_status(&f), O::Ready {});
    }
    #[test]
    fn v5_read_target_reuses_saved_revision_but_requires_matching_completed_result() {
        use studio_runtime_schema::{AgentCounter, agent_entity_nested_extra::ReadStateDto};
        let receipt = ReadStateDto {
            thread_id: "old-thread".into(),
            turn_id: "old-turn".into(),
            read: true,
            revision: AgentCounter::from_i64(7).expect("revision"),
        };
        let target = mark_read_target(
            Some("current"),
            Some("turn"),
            Some("completed"),
            Some(&receipt),
        )
        .expect("target");
        assert!(!target.read);
        assert_eq!(target.revision.as_i64(), 7);
        assert!(
            mark_read_target(
                Some("current"),
                Some("turn"),
                Some("failed"),
                Some(&receipt)
            )
            .is_none()
        );
        let read = ReadStateDto {
            thread_id: "current".into(),
            turn_id: "turn".into(),
            ..receipt
        };
        assert!(
            mark_read_target(
                Some("current"),
                Some("turn"),
                Some("completed"),
                Some(&read)
            )
            .expect("target")
            .read
        );
    }
}
