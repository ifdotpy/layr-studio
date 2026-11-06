//! Identifies transient command progress on the live stream.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgressSelector {
    Auth {
        request_id: String,
    },
    Rename {
        request_id: String,
        agent_id: String,
    },
}
