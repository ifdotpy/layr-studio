//! Typed identities for resources on the live stream.

use crate::{
    bounded_integer::BoundedInteger, bounded_text::BoundedText, progress_selector::VaultJob,
};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error, ser::SerializeMap};

/// Maximum auth-progress request identity, in Unicode code points.
pub const AUTH_REQUEST_ID_MAX: usize = 256;
/// Maximum vault-job request identity, in Unicode code points.
pub const VAULT_JOB_REQUEST_ID_MAX: usize = 256;
/// Maximum rename-progress request identity, in Unicode code points.
pub const RENAME_REQUEST_ID_MAX: usize = 200;
/// Maximum rename-progress agent identity, in Unicode code points.
pub const RENAME_AGENT_ID_MAX: usize = 200;
/// Maximum vault prefix length, in ASCII bytes.
pub const VAULT_PREFIX_MAX: usize = 44;

/// An identifier whose schema permits any nonempty string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct NonEmpty(String);

impl NonEmpty {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Display for NonEmpty {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}
impl TryFrom<String> for NonEmpty {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err("empty resource identifier")
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> Deserialize<'de> for NonEmpty {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::try_from(String::deserialize(d)?).map_err(D::Error::custom)
    }
}
impl std::ops::Deref for NonEmpty {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}
impl JsonSchema for NonEmpty {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "NonEmptyResourceId".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"type":"string","minLength":1})
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EmptyResource {}

macro_rules! resource {
    ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Clone, Debug, Deserialize, JsonSchema)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        pub struct $name { $(pub $field: $ty),* }
    };
}
resource!(AgentResource { agent_id: NonEmpty });
resource!(AccountResource {
    account_key: NonEmpty
});
resource!(TaskResource { task_id: NonEmpty });
resource!(TerminalResource {
    terminal_id: NonEmpty
});
resource!(ComplaintResource {
    complaint_id: NonEmpty
});
resource!(RequestResource { request_id: BoundedText<1, AUTH_REQUEST_ID_MAX> });
resource!(RenameResource { agent_id: BoundedText<1, RENAME_AGENT_ID_MAX>, request_id: BoundedText<1, RENAME_REQUEST_ID_MAX> });
resource!(VaultJobResource { job: VaultJob, request_id: BoundedText<1, VAULT_JOB_REQUEST_ID_MAX> });

fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OptionalAgentResource {
    #[serde(default, deserialize_with = "present")]
    pub agent_id: Option<NonEmpty>,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoomResource {
    #[serde(default, deserialize_with = "present")]
    pub legacy: Option<bool>,
    pub room_id: NonEmpty,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TranscriptResource {
    pub agent_id: NonEmpty,
    #[serde(default, deserialize_with = "present")]
    pub tail: Option<
        BoundedInteger<1, { crate::shared_values::SYNC_TRANSCRIPT_STREAM_MAX_ITEMS as u64 }>,
    >,
}

/// Vault prefix used by resource subscriptions.
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct VaultPrefix(String);
impl VaultPrefix {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
/// Validate the vault prefix used for directories and subscriptions.
pub fn valid_vault_prefix(value: &str) -> bool {
    let mut bytes = value.bytes();
    value.len() <= VAULT_PREFIX_MAX
        && bytes.next().is_some_and(|b| b.is_ascii_alphanumeric())
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
impl<'de> Deserialize<'de> for VaultPrefix {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        if !valid_vault_prefix(&value) {
            return Err(D::Error::custom("invalid vault prefix"));
        }
        Ok(Self(value))
    }
}
impl JsonSchema for VaultPrefix {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "VaultResourcePrefix".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"type":"string","minLength":1,"maxLength":VAULT_PREFIX_MAX,"pattern":"^[A-Za-z0-9][A-Za-z0-9_-]*$"})
    }
}
resource!(VaultReadinessResource {
    prefix: VaultPrefix
});

/// Closed, tagged wire union. Payload fields deliberately exclude the tag.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(tag = "kind")]
pub enum ResourceRef {
    #[serde(rename = "accounts")]
    AccountsResource(EmptyResource),
    #[serde(rename = "costs")]
    CostsResource(EmptyResource),
    #[serde(rename = "desktop")]
    DesktopResource(EmptyResource),
    #[serde(rename = "drafts")]
    DraftsResource(EmptyResource),
    #[serde(rename = "limits")]
    LimitsResource(AccountResource),
    #[serde(rename = "models")]
    ModelsResource(EmptyResource),
    #[serde(rename = "multi-server")]
    MultiServerResource(EmptyResource),
    #[serde(rename = "queue")]
    QueueResource(AgentResource),
    #[serde(rename = "receipts")]
    ReceiptsResource(AgentResource),
    #[serde(rename = "room")]
    RoomResource(RoomResource),
    #[serde(rename = "session-cost")]
    SessionCostResource(AgentResource),
    #[serde(rename = "state")]
    StateResource(EmptyResource),
    #[serde(rename = "task")]
    TaskResource(TaskResource),
    #[serde(rename = "tasks")]
    TasksResource(OptionalAgentResource),
    #[serde(rename = "terminal")]
    TerminalResource(TerminalResource),
    #[serde(rename = "terminals")]
    TerminalsResource(EmptyResource),
    #[serde(rename = "transcript")]
    TranscriptResource(TranscriptResource),
    #[serde(rename = "transcripts")]
    TranscriptsResource(EmptyResource),
    #[serde(rename = "voice")]
    VoiceResource(AgentResource),
    #[serde(rename = "workspace")]
    WorkspaceResource(AgentResource),
    #[serde(rename = "auth-progress")]
    AuthProgressResource(RequestResource),
    #[serde(rename = "vault-readiness")]
    VaultReadinessResource(VaultReadinessResource),
    #[serde(rename = "vault-job-progress")]
    VaultJobProgressResource(VaultJobResource),
    #[serde(rename = "rename-progress")]
    RenameProgressResource(RenameResource),
    #[serde(rename = "agentDetail")]
    AgentDetailResource(AgentResource),
    #[serde(rename = "monitors")]
    MonitorsResource(OptionalAgentResource),
    #[serde(rename = "processes")]
    ProcessesResource(AgentResource),
    #[serde(rename = "work")]
    WorkResource(AgentResource),
    #[serde(rename = "complaint")]
    ComplaintResource(ComplaintResource),
    #[serde(rename = "terminal-output")]
    TerminalOutputResource(TerminalResource),
}

// Resource keys are JSON object bytes, so emit fields in lexical order.
impl Serialize for ResourceRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        macro_rules! kind {
            ($value:expr) => {
                map.serialize_entry("kind", $value)?
            };
        }
        macro_rules! agent {
            ($value:expr, $kind:expr) => {{
                map.serialize_entry("agentId", &$value.agent_id)?;
                kind!($kind);
            }};
        }
        match self {
            Self::AccountsResource(_) => kind!("accounts"),
            Self::CostsResource(_) => kind!("costs"),
            Self::DesktopResource(_) => kind!("desktop"),
            Self::DraftsResource(_) => kind!("drafts"),
            Self::LimitsResource(v) => {
                map.serialize_entry("accountKey", &v.account_key)?;
                kind!("limits");
            }
            Self::ModelsResource(_) => kind!("models"),
            Self::MultiServerResource(_) => kind!("multi-server"),
            Self::QueueResource(v) => agent!(v, "queue"),
            Self::ReceiptsResource(v) => agent!(v, "receipts"),
            Self::RoomResource(v) => {
                kind!("room");
                if let Some(legacy) = v.legacy {
                    map.serialize_entry("legacy", &legacy)?;
                }
                map.serialize_entry("roomId", &v.room_id)?;
            }
            Self::SessionCostResource(v) => agent!(v, "session-cost"),
            Self::StateResource(_) => kind!("state"),
            Self::TaskResource(v) => {
                kind!("task");
                map.serialize_entry("taskId", &v.task_id)?;
            }
            Self::TasksResource(v) => {
                if let Some(agent) = &v.agent_id {
                    map.serialize_entry("agentId", agent)?;
                }
                kind!("tasks");
            }
            Self::TerminalResource(v) => {
                kind!("terminal");
                map.serialize_entry("terminalId", &v.terminal_id)?;
            }
            Self::TerminalsResource(_) => kind!("terminals"),
            Self::TranscriptResource(v) => {
                agent!(v, "transcript");
                if let Some(tail) = v.tail {
                    map.serialize_entry("tail", &tail)?;
                }
            }
            Self::TranscriptsResource(_) => kind!("transcripts"),
            Self::VoiceResource(v) => agent!(v, "voice"),
            Self::WorkspaceResource(v) => agent!(v, "workspace"),
            Self::AuthProgressResource(v) => {
                kind!("auth-progress");
                map.serialize_entry("requestId", &v.request_id)?;
            }
            Self::VaultReadinessResource(v) => {
                kind!("vault-readiness");
                map.serialize_entry("prefix", &v.prefix)?;
            }
            Self::VaultJobProgressResource(v) => {
                map.serialize_entry("job", &v.job)?;
                kind!("vault-job-progress");
                map.serialize_entry("requestId", &v.request_id)?;
            }
            Self::RenameProgressResource(v) => {
                map.serialize_entry("agentId", &v.agent_id)?;
                kind!("rename-progress");
                map.serialize_entry("requestId", &v.request_id)?;
            }
            Self::AgentDetailResource(v) => agent!(v, "agentDetail"),
            Self::MonitorsResource(v) => {
                if let Some(agent) = &v.agent_id {
                    map.serialize_entry("agentId", agent)?;
                }
                kind!("monitors");
            }
            Self::ProcessesResource(v) => agent!(v, "processes"),
            Self::WorkResource(v) => agent!(v, "work"),
            Self::ComplaintResource(v) => {
                map.serialize_entry("complaintId", &v.complaint_id)?;
                kind!("complaint");
            }
            Self::TerminalOutputResource(v) => {
                kind!("terminal-output");
                map.serialize_entry("terminalId", &v.terminal_id)?;
            }
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::ResourceRef;
    use serde_json::json;

    #[test]
    fn resource_ref_matches_schema_and_preserves_canonical_key() {
        let validator = crate::compiled_validator("ResourceRef").expect("resource schema");
        let document: serde_json::Value =
            serde_json::from_str(include_str!("../../runtime-http/openapi.json")).expect("OpenAPI");
        for (path, limit) in [
            (
                "/components/schemas/AuthProgressResource/properties/requestId/maxLength",
                super::AUTH_REQUEST_ID_MAX,
            ),
            (
                "/components/schemas/VaultJobProgressResource/properties/requestId/maxLength",
                super::VAULT_JOB_REQUEST_ID_MAX,
            ),
            (
                "/components/schemas/RenameProgressResource/properties/requestId/maxLength",
                super::RENAME_REQUEST_ID_MAX,
            ),
            (
                "/components/schemas/RenameProgressResource/properties/agentId/maxLength",
                super::RENAME_AGENT_ID_MAX,
            ),
            (
                "/components/schemas/VaultReadinessResource/properties/prefix/maxLength",
                super::VAULT_PREFIX_MAX,
            ),
        ] {
            assert_eq!(document.pointer(path), Some(&json!(limit)), "{path}");
        }
        for value in [
            json!({"kind":"accounts"}),
            json!({"kind":"costs"}),
            json!({"kind":"desktop"}),
            json!({"kind":"drafts"}),
            json!({"kind":"limits","accountKey":"account"}),
            json!({"kind":"models"}),
            json!({"kind":"multi-server"}),
            json!({"kind":"queue","agentId":"agent"}),
            json!({"kind":"receipts","agentId":"agent"}),
            json!({"kind":"room","roomId":"room","legacy":true}),
            json!({"kind":"session-cost","agentId":"agent"}),
            json!({"kind":"state"}),
            json!({"kind":"task","taskId":"task"}),
            json!({"kind":"terminal","terminalId":"terminal"}),
            json!({"kind":"terminals"}),
            json!({"kind":"transcript","agentId":"agent","tail":40}),
            json!({"kind":"transcripts"}),
            json!({"kind":"voice","agentId":"agent"}),
            json!({"kind":"workspace","agentId":"agent"}),
            json!({"kind":"auth-progress","requestId":"request"}),
            json!({"kind":"vault-readiness","prefix":"vault-1"}),
            json!({"kind":"vault-job-progress","requestId":"request","job":"pull"}),
            json!({"kind":"rename-progress","requestId":"request","agentId":"agent"}),
            json!({"kind":"agentDetail","agentId":"agent"}),
            json!({"kind":"monitors","agentId":"agent"}),
            json!({"kind":"processes","agentId":"agent"}),
            json!({"kind":"work","agentId":"agent"}),
            json!({"kind":"complaint","complaintId":"complaint"}),
            json!({"kind":"terminal-output","terminalId":"terminal"}),
            json!({"kind":"tasks"}),
            json!({"kind":"tasks","agentId":null}),
            json!({"kind":"accounts","extra":true}),
            json!({"kind":"transcript","agentId":"","tail":40}),
            json!({"kind":"transcript","agentId":"agent","tail":513}),
            json!({"kind":"vault-readiness","prefix":"bad/path"}),
        ] {
            let typed = serde_json::from_str::<ResourceRef>(&value.to_string());
            assert_eq!(typed.is_ok(), validator.is_valid(&value), "{value}");
            if let Ok(typed) = typed {
                assert_eq!(
                    serde_json::to_string(&typed).expect("serialize"),
                    value.to_string()
                );
            }
        }
    }
}
