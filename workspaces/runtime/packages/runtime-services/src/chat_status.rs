//! Server-owned operational chat indicator, excluding device-local unread state.
pub use studio_runtime_schema::chat_operational_status::{
    ChatOperationalKind, ChatOperationalStatus,
};
use studio_runtime_store::AgentStatus;

#[allow(clippy::struct_excessive_bools)] // Independent runtime facts feed status precedence.
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
    /// A live wait's already derived description, or none when the wait is not live.
    pub wait_label: Option<&'a str>,
}

pub fn derive_chat_status(facts: &ChatStatusFacts<'_>) -> ChatOperationalStatus {
    use AgentStatus as S;
    use ChatOperationalKind as K;
    let waiting = matches!(
        facts.status,
        Some(S::Waiting | S::Parked | S::WaitingOnMonitor)
    );
    let (kind, label) = if facts.answer {
        (
            K::Answer,
            if waiting {
                facts.wait_label.unwrap_or("Needs your answer")
            } else {
                "Needs your answer"
            },
        )
    } else if waiting && facts.background_work {
        (K::Working, "Waiting for background command")
    } else if waiting {
        match facts.wait_label {
            Some(label) => (K::Working, label),
            None => (K::None, "Turn ended. Send a message to continue."),
        }
    } else if facts.active {
        (
            K::Working,
            if !facts.in_flight {
                facts.wait_label.unwrap_or("Working")
            } else {
                "Working"
            },
        )
    } else if matches!(facts.status, Some(S::Failed)) {
        if facts.disk_full {
            (K::Paused, "Waiting for disk space")
        } else {
            (K::Error, "Failed")
        }
    } else if matches!(facts.status, Some(S::Paused | S::Interrupted))
        || (facts.auto_wake == Some(false) && !facts.empty)
    {
        (
            K::Paused,
            if matches!(facts.status, Some(S::Interrupted)) {
                "Interrupted"
            } else {
                "Stopped"
            },
        )
    } else if facts.deferred && matches!(facts.status, Some(S::Approval)) {
        (K::Paused, "Question deferred")
    } else if matches!(facts.status, Some(S::Completed)) {
        (K::None, "Read")
    } else {
        (K::None, "Ready")
    };
    ChatOperationalStatus {
        kind,
        label: label.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operational_status_preserves_precedence_and_live_waits() {
        let mut facts = ChatStatusFacts {
            status: Some(&AgentStatus::Waiting),
            auto_wake: Some(true),
            empty: false,
            in_flight: false,
            answer: true,
            deferred: false,
            background_work: true,
            active: true,
            disk_full: false,
            wait_label: Some("Waiting for 1 agent"),
        };
        assert_eq!(
            derive_chat_status(&facts),
            ChatOperationalStatus {
                kind: ChatOperationalKind::Answer,
                label: "Waiting for 1 agent".into()
            }
        );
        facts.answer = false;
        assert_eq!(
            derive_chat_status(&facts).label,
            "Waiting for background command"
        );
        facts.background_work = false;
        assert_eq!(
            derive_chat_status(&facts).kind,
            ChatOperationalKind::Working
        );
        facts.wait_label = None;
        assert_eq!(
            derive_chat_status(&facts),
            ChatOperationalStatus {
                kind: ChatOperationalKind::None,
                label: "Turn ended. Send a message to continue.".into()
            }
        );
        facts.status = Some(&AgentStatus::Failed);
        facts.active = false;
        facts.disk_full = true;
        assert_eq!(
            derive_chat_status(&facts),
            ChatOperationalStatus {
                kind: ChatOperationalKind::Paused,
                label: "Waiting for disk space".into()
            }
        );
        facts.disk_full = false;
        assert_eq!(derive_chat_status(&facts).kind, ChatOperationalKind::Error);
        facts.status = Some(&AgentStatus::Completed);
        assert_eq!(derive_chat_status(&facts).label, "Read");
    }

    #[test]
    fn operational_status_covers_deferral_stops_and_active_work() {
        let mut facts = ChatStatusFacts {
            status: Some(&AgentStatus::Approval),
            auto_wake: Some(true),
            empty: false,
            in_flight: false,
            answer: false,
            deferred: true,
            background_work: false,
            active: false,
            disk_full: false,
            wait_label: None,
        };
        assert_eq!(derive_chat_status(&facts).label, "Question deferred");
        facts.status = Some(&AgentStatus::Interrupted);
        assert_eq!(derive_chat_status(&facts).label, "Interrupted");
        facts.status = Some(&AgentStatus::Idle);
        facts.auto_wake = Some(false);
        assert_eq!(derive_chat_status(&facts).label, "Stopped");
        facts.empty = true;
        assert_eq!(derive_chat_status(&facts).label, "Ready");
        facts.status = Some(&AgentStatus::Failed);
        facts.active = true;
        facts.wait_label = Some("Waiting for 1 monitor");
        assert_eq!(derive_chat_status(&facts).label, "Waiting for 1 monitor");
        facts.in_flight = true;
        assert_eq!(derive_chat_status(&facts).label, "Working");
    }
}
