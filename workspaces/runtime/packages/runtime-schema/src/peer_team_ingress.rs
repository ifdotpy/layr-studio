//! Peer-team and shared-chat mutation requests.
use crate::bounded_text::BoundedText;
use crate::queue_command::Field;
use crate::recovery_record::SafeInteger;
use serde::{Deserialize, Deserializer, Serialize};

const RADIO_MIN_ROUNDS: f64 = 1.0;
const RADIO_MAX_ROUNDS: f64 = 2.0;

#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct Rounds(SafeInteger);
impl Rounds {
    pub fn as_u64(&self) -> Option<u64> {
        self.0.as_u64()
    }
}
impl<'de> Deserialize<'de> for Rounds {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = SafeInteger::deserialize(deserializer)?;
        if (RADIO_MIN_ROUNDS..=RADIO_MAX_ROUNDS).contains(&value.get()) {
            Ok(Self(value))
        } else {
            Err(serde::de::Error::custom("rounds must be one or two"))
        }
    }
}
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SaveAction {
    Save,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DeleteAction {
    Delete,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MoveAction {
    Move,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConvertAction {
    Convert,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Save {
    pub action: SaveAction,
    pub path: String,
    pub expected_revision: SafeInteger,
    pub request_id: String,
    pub name: String,
    pub members: Vec<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub team_id: Field<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Delete {
    pub action: DeleteAction,
    pub path: String,
    pub expected_revision: SafeInteger,
    pub request_id: String,
    pub team_id: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Move {
    pub action: MoveAction,
    pub path: String,
    pub expected_revision: SafeInteger,
    pub request_id: String,
    pub member: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub team_id: Field<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Convert {
    pub action: ConvertAction,
    pub path: String,
    pub expected_revision: SafeInteger,
    pub request_id: String,
    pub member: String,
    pub target: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Participant {
    pub account_key: BoundedText<1, 255>,
    pub model: BoundedText<1, 255>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub effort: Field<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RadioAction {
    Radio,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CreateRadioAction {
    Create,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OpenRadioAction {
    Open,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SendRadioAction {
    Send,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PassRadioAction {
    Pass,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StopRadioAction {
    Stop,
}

fn participants<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[Participant; 2], D::Error> {
    let [first, second] =
        <[crate::http_ingress::Object<Participant>; 2]>::deserialize(deserializer)?;
    Ok([first.0, second.0])
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RadioCreate {
    pub action: RadioAction,
    pub radio_action: CreateRadioAction,
    pub path: BoundedText<1, 4096>,
    pub request_id: BoundedText<1, 255>,
    pub name: BoundedText<1, 80>,
    #[serde(deserialize_with = "participants")]
    pub participants: [Participant; 2],
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RadioOpen {
    pub action: RadioAction,
    pub radio_action: OpenRadioAction,
    pub path: String,
    pub team_id: String,
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub expected_revision: Field<SafeInteger>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RadioSend {
    pub action: RadioAction,
    pub radio_action: SendRadioAction,
    pub path: String,
    pub team_id: String,
    pub request_id: String,
    pub expected_revision: SafeInteger,
    pub text: String,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub target: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub rounds: Option<Rounds>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RadioPass {
    pub action: RadioAction,
    pub radio_action: PassRadioAction,
    pub path: String,
    pub team_id: String,
    pub request_id: String,
    pub expected_revision: SafeInteger,
    pub target: String,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub rounds: Option<Rounds>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RadioStop {
    pub action: RadioAction,
    pub radio_action: StopRadioAction,
    pub path: String,
    pub team_id: String,
    pub request_id: String,
    pub expected_revision: SafeInteger,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PeerTeamRequest {
    Save(Save),
    Delete(Delete),
    Move(Move),
    Convert(Convert),
    RadioCreate(RadioCreate),
    RadioOpen(RadioOpen),
    RadioSend(RadioSend),
    RadioPass(RadioPass),
    RadioStop(RadioStop),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TeamMutation {
    Save(Save),
    Delete(Delete),
    Move(Move),
}
impl TeamMutation {
    pub fn path(&self) -> &str {
        match self {
            Self::Save(v) => &v.path,
            Self::Delete(v) => &v.path,
            Self::Move(v) => &v.path,
        }
    }
    pub fn request_id(&self) -> &str {
        match self {
            Self::Save(v) => &v.request_id,
            Self::Delete(v) => &v.request_id,
            Self::Move(v) => &v.request_id,
        }
    }
    pub fn expected_revision(&self) -> &SafeInteger {
        match self {
            Self::Save(v) => &v.expected_revision,
            Self::Delete(v) => &v.expected_revision,
            Self::Move(v) => &v.expected_revision,
        }
    }
    pub fn team_id(&self) -> Option<&str> {
        match self {
            Self::Save(v) => v.team_id.value().map(String::as_str),
            Self::Delete(v) => Some(&v.team_id),
            Self::Move(v) => v.team_id.value().map(String::as_str),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RadioManage {
    Open(RadioOpen),
    Send(RadioSend),
    Pass(RadioPass),
    Stop(RadioStop),
}
impl RadioManage {
    pub fn path(&self) -> &str {
        match self {
            Self::Open(v) => &v.path,
            Self::Send(v) => &v.path,
            Self::Pass(v) => &v.path,
            Self::Stop(v) => &v.path,
        }
    }
    pub fn request_id(&self) -> &str {
        match self {
            Self::Open(v) => &v.request_id,
            Self::Send(v) => &v.request_id,
            Self::Pass(v) => &v.request_id,
            Self::Stop(v) => &v.request_id,
        }
    }
    pub fn team_id(&self) -> &str {
        match self {
            Self::Open(v) => &v.team_id,
            Self::Send(v) => &v.team_id,
            Self::Pass(v) => &v.team_id,
            Self::Stop(v) => &v.team_id,
        }
    }
    pub fn revision(&self) -> Option<u64> {
        match self {
            Self::Open(v) => v.expected_revision.value().and_then(SafeInteger::as_u64),
            Self::Send(v) => v.expected_revision.as_u64(),
            Self::Pass(v) => v.expected_revision.as_u64(),
            Self::Stop(v) => v.expected_revision.as_u64(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PeerTeamRequest;
    use serde::Deserialize;

    #[test]
    fn send_rounds_are_bounded_and_non_nullable() {
        let base = serde_json::json!({"action":"radio","radio_action":"send","path":"p","team_id":"t","request_id":"r","expected_revision":1,"text":"hi"});
        assert!(PeerTeamRequest::deserialize(base.clone()).is_ok());
        for rounds in [
            serde_json::json!(null),
            serde_json::json!(0),
            serde_json::json!(3),
        ] {
            let mut bad = base.clone();
            bad["rounds"] = rounds;
            assert!(PeerTeamRequest::deserialize(bad).is_err());
        }
    }
}
