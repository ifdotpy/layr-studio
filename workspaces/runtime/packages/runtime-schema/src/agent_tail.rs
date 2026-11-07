//! Tiny read-time lead search tail; worker tails are deliberately excluded.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
/// A lead's committed tail at the existing provider flush/completion cadence.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentTail {
    /// Lead agent identity; workers do not produce rows in this collection.
    pub id: String,
    /// Search excerpt, bounded to Unicode scalar values; empty when no tail exists.
    pub tail: crate::bounded_text::BoundedText<0, { crate::shared_values::AGENT_TAIL_MAX_CHARS }>,
}
pub const COLLECTION: &str = "agentTail";
pub(crate) fn refresh_openapi(openapi: &mut studio_json::Value) {
    use studio_json::json;
    let s = &mut openapi["components"]["schemas"];
    s["SyncAgentTailEntityPayload"] = json!({"type":"object","additionalProperties":false,"properties":{"collection":{"type":"string","const":COLLECTION},"id":{"type":"string"},"value":{"$ref":"#/components/schemas/AgentTail"}},"required":["collection","id","value"]});
    s["SyncEntityPayload"]["discriminator"]["mapping"][COLLECTION] =
        json!("#/components/schemas/SyncAgentTailEntityPayload");
    let r = json!({"$ref":"#/components/schemas/SyncAgentTailEntityPayload"});
    let variants = s["SyncEntityPayload"]["oneOf"]
        .as_array_mut()
        .expect("payload variants");
    if !variants.contains(&r) {
        variants.push(r);
    }
    let variants = s["SyncEntity"]["oneOf"]
        .as_array_mut()
        .expect("entity variants");
    if !variants
        .iter()
        .any(|v| v["properties"]["collection"]["const"] == COLLECTION)
    {
        variants.push(json!({"type":"object","properties":{"op":{"type":"string","const":"upsert"},"collection":{"type":"string","const":COLLECTION},"value":{"$ref":"#/components/schemas/AgentTail"}},"required":["op","collection","value"]}));
    }
    for v in variants {
        if v["properties"]["op"]["const"] == "delete" {
            let c = v["properties"]["collection"]["enum"]
                .as_array_mut()
                .expect("delete kinds");
            if !c.contains(&json!(COLLECTION)) {
                c.push(json!(COLLECTION));
            }
        }
    }
    for name in ["StateResource", "GlobalStateResource"] {
        if let Some(c) = s[name]["x-studio-entity-scope"]["collections"].as_array_mut() {
            if !c.contains(&json!(COLLECTION)) {
                c.push(json!(COLLECTION));
            }
        }
    }
}
