//! Controls derived from the stored radio state, independent of device drafts.
use crate::room_record::Status;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)] // Independent controls are published to clients.
pub struct RadioControls {
    pub busy: bool,
    pub input_blocked: bool,
    pub can_pass: bool,
    pub stopping: bool,
    pub speaking: bool,
    pub busy_notice: Option<String>,
}

impl RadioControls {
    pub fn derive(status: Status, active: bool) -> Self {
        let busy = active || !matches!(status, Status::Idle | Status::Blocked);
        let stopping = status == Status::Stopping;
        Self {
            busy,
            input_blocked: stopping || (status == Status::Blocked && active),
            can_pass: busy && status != Status::Blocked,
            stopping,
            speaking: status == Status::Speaking,
            busy_notice: busy.then(|| {
                match status {
                    Status::Blocked => "Resolve the current reply before sending.",
                    Status::Stopping => "Waiting for the reply to stop.",
                    Status::Idle | Status::Waiting | Status::Speaking => {
                        "Current reply finishes first."
                    }
                }
                .to_owned()
            }),
        }
    }
}

pub(crate) fn schema() -> serde_json::Value {
    let mut generator = schemars::generate::SchemaSettings::openapi3()
        .for_serialize()
        .into_generator();
    RadioControls::json_schema(&mut generator).to_value()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_cover_radio_states_and_active_block() {
        for (status, active, busy, blocked, pass, stopping, speaking) in [
            (Status::Idle, false, false, false, false, false, false),
            (Status::Waiting, false, true, false, true, false, false),
            (Status::Speaking, true, true, false, true, false, true),
            (Status::Blocked, false, false, false, false, false, false),
            (Status::Blocked, true, true, true, false, false, false),
            (Status::Stopping, true, true, true, true, true, false),
        ] {
            let value = RadioControls::derive(status, active);
            assert_eq!(
                (
                    value.busy,
                    value.input_blocked,
                    value.can_pass,
                    value.stopping,
                    value.speaking
                ),
                (busy, blocked, pass, stopping, speaking)
            );
            assert_eq!(value.busy_notice.is_some(), busy);
        }
    }
}
