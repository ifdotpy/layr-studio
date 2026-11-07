//! Server-owned chat indicator, including persisted unread receipt precedence.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChatOperationalKind {
    Answer,
    Error,
    Working,
    Unread,
    Paused,
    None,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChatOperationalStatus {
    pub kind: ChatOperationalKind,
    pub label: String,
}

pub(crate) fn schemas() -> Vec<(&'static str, studio_json::Value)> {
    let mut generator = schemars::generate::SchemaSettings::openapi3()
        .for_serialize()
        .into_generator();
    vec![
        (
            "ChatOperationalKind",
            ChatOperationalKind::json_schema(&mut generator).to_value(),
        ),
        (
            "ChatOperationalStatus",
            ChatOperationalStatus::json_schema(&mut generator).to_value(),
        ),
    ]
}

#[cfg(test)]
mod v5_tests {
    use super::*;
    #[test]
    fn operational_waits_are_nonempty_and_reason_payloads_have_no_labels() {
        let empty = studio_json::json!({"reason":"waiting","wait":{"agents":0,"commands":0,"monitors":0,"inputs":0,"event":null}});
        assert!(studio_json::from_value::<AgentOperationalStatus>(empty).is_err());
        let live = studio_json::json!({"reason":"waiting","wait":{"agents":1,"commands":0,"monitors":0,"inputs":0,"event":null}});
        let status: AgentOperationalStatus =
            studio_json::from_value(live.clone()).expect("live wait");
        assert_eq!(studio_json::to_value(status).expect("wire"), live);
        assert!(
            studio_json::from_value::<AgentOperationalStatus>(
                studio_json::json!({"reason":"working","label":"Working"})
            )
            .is_err()
        );
    }
}

/// Raw domain wait counts. Units are live processes/inputs, never display strings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaitReasonFields {
    pub agents: u32,
    pub commands: u32,
    pub monitors: u32,
    pub inputs: u32,
    pub event: Option<String>,
}

/// A live wait has at least one positive count or a nonempty event identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentWaitReasons(#[serde(deserialize_with = "checked_wait")] WaitReasonFields);
fn checked_wait<'de, D: serde::Deserializer<'de>>(d: D) -> Result<WaitReasonFields, D::Error> {
    let fields = WaitReasonFields::deserialize(d)?;
    AgentWaitReasons::new(fields)
        .map(|v| v.0)
        .ok_or_else(|| serde::de::Error::custom("wait reasons must not be empty"))
}
impl AgentWaitReasons {
    pub fn new(fields: WaitReasonFields) -> Option<Self> {
        (fields.event.as_ref().is_none_or(|event| !event.is_empty())
            && (fields.agents > 0
                || fields.commands > 0
                || fields.monitors > 0
                || fields.inputs > 0
                || fields.event.is_some()))
        .then_some(Self(fields))
    }
    pub fn facts(&self) -> &WaitReasonFields {
        &self.0
    }
}
impl JsonSchema for AgentWaitReasons {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "AgentWaitReasons".into()
    }
    fn json_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let mut schema = WaitReasonFields::json_schema(g);
        if let Some(properties) = schema.get_mut("properties").and_then(|v| v.as_object_mut()) {
            properties.insert(
                "event".into(),
                studio_json::json!({"anyOf":[{"type":"string","minLength":1},{"type":"null"}]}),
            );
        }
        schema.insert(
            "required".into(),
            studio_json::json!(["agents", "commands", "monitors", "inputs", "event"]),
        );
        let base = schema.clone().to_value();
        let mut variants = Vec::new();
        for field in ["agents", "commands", "monitors", "inputs", "event"] {
            let mut variant = base.clone();
            if field == "event" {
                variant["properties"][field] = studio_json::json!({"type":"string","minLength":1});
            } else {
                variant["properties"][field]["minimum"] = studio_json::json!(1);
            }
            variants.push(variant);
        }
        schema.insert("anyOf".into(), studio_json::json!(variants));
        schema
    }
}

/// Domain operational precedence; presentation belongs to the client formatter.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "reason", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentOperationalStatus {
    NeedsInput { wait: Option<AgentWaitReasons> },
    Waiting { wait: AgentWaitReasons },
    WaitingBackgroundCommand {},
    Working {},
    TurnEnded {},
    DiskSpace {},
    Failed {},
    Interrupted {},
    Stopped {},
    QuestionDeferred {},
    UnreadResult {},
    ReadResult {},
    Ready {},
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarkUnreadEligibility {
    Allowed,
    Unsupported,
    NoCompletedResult,
    AlreadyUnread,
}

/// Extend only the existing summary's domain facts, preserving its other consumers.
pub(crate) fn refresh_summary(openapi: &mut studio_json::Value) {
    use studio_json::json;
    let mut g = schemars::generate::SchemaSettings::draft2020_12()
        .for_serialize()
        .with(|s| s.definitions_path = "/components/schemas".into())
        .into_generator();
    g.subschema_for::<AgentOperationalStatus>();
    g.subschema_for::<MarkUnreadEligibility>();
    g.subschema_for::<crate::agent_tail::AgentTail>();
    for (name, schema) in g.take_definitions(true) {
        openapi["components"]["schemas"][name] = schema;
    }
    let summary = &mut openapi["components"]["schemas"]["AgentSummaryEntityDto"];
    summary["properties"]["operationalStatus"] =
        json!({"$ref":"#/components/schemas/AgentOperationalStatus"});
    summary["properties"]["markRead"] =
        json!({"anyOf":[{"$ref":"#/components/schemas/ReadStateDto"},{"type":"null"}]});
    summary["properties"]["markUnread"] =
        json!({"$ref":"#/components/schemas/MarkUnreadEligibility"});
    summary["properties"]["inFlight"] = json!({"type":"boolean"});
    summary["properties"]["projectServerId"] = json!({"anyOf":[{"type":"string"},{"type":"null"}]});
    for field in [
        "operationalStatus",
        "markRead",
        "markUnread",
        "inFlight",
        "projectServerId",
    ] {
        let required = summary["required"]
            .as_array_mut()
            .expect("summary required");
        if !required.contains(&json!(field)) {
            required.push(json!(field));
        }
    }
    crate::agent_tail::refresh_openapi(openapi);
}
