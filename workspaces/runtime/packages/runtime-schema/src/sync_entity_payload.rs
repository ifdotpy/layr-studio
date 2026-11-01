//! Serde owners for migrated sync entity payload variants.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "collection", rename_all = "camelCase", deny_unknown_fields)]
pub enum SyncEntityPayload {
    Edge {
        id: String,
        value: EdgeEntity,
    },
    Chat {
        id: String,
        value: ChatEntity,
    },
    WorkRelease {
        id: String,
        value: WorkReleaseEntity,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "EdgeEntityDto")]
pub struct EdgeEntity {
    pub id: String,
    #[schemars(default)]
    pub kind: Option<EdgeKind>,
    #[schemars(default)]
    pub source: Option<String>,
    #[schemars(default)]
    pub target: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schemars(rename = "ChatEntityDto")]
pub struct ChatEntity {
    pub id: String,
    pub kind: Option<ChatKind>,
    pub last_message_at: Option<f64>,
    pub members: Option<Vec<String>>,
    #[schemars(with = "Option<u64>", range(max = crate::shared_values::MAX_SAFE_INTEGER))]
    pub message_count: Option<crate::recovery_record::SafeInteger>,
    pub name: Option<String>,
    pub tail: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChatKind {
    Chat,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schemars(rename = "WorkReleaseEntityDto")]
pub struct WorkReleaseEntity {
    pub id: String,
    pub task_id: String,
    pub root_id: String,
    #[schemars(with = "u64")]
    pub index: crate::recovery_record::NonnegativeInteger,
    #[schemars(schema_with = "json_value_schema")]
    pub value: serde_json::Value,
}

fn json_value_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"$ref":"#/components/schemas/JsonValue"})
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    Chat,
    Spawn,
}

/// Render migrated variant schemas into the existing union components.
pub(crate) fn schemas() -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let mut generator = schemars::generate::SchemaSettings::draft2020_12()
        .with(|settings| settings.definitions_path = "/components/schemas".into())
        .into_generator();
    let schema = SyncEntityPayload::json_schema(&mut generator).to_value();
    let mut schemas = generator.take_definitions(true);
    let variants = schema
        .get("oneOf")
        .and_then(serde_json::Value::as_array)
        .ok_or("missing payload variants")?;
    for variant in variants {
        let collection = variant
            .pointer("/properties/collection/const")
            .and_then(serde_json::Value::as_str)
            .ok_or("missing payload collection")?;
        let mut chars = collection.chars();
        let first = chars.next().ok_or("empty payload collection")?;
        let name = format!(
            "Sync{}{}EntityPayload",
            first.to_ascii_uppercase(),
            chars.as_str()
        );
        schemas.insert(name, variant.clone());
    }
    Ok(schemas)
}

#[cfg(test)]
mod tests {
    use super::SyncEntityPayload;
    use serde_json::json;

    #[test]
    fn edge_previous_wire_fixtures() {
        // Frozen cases from the pre-migration edge contract, independent of generated schemas.
        for (value, valid) in [
            (json!({"id":"e"}), true),
            (
                json!({"id":"e","kind":"chat","source":null,"target":"t"}),
                true,
            ),
            (json!({"id":"e","kind":"spawn"}), true),
            (json!({"id":"e","kind":null}), true),
            (json!({"id":"e","kind":"unknown"}), false),
            (json!({"id":"e","unexpected":true}), false),
            (json!({}), false),
        ] {
            let payload = json!({"collection":"edge","id":"e","value":value});
            assert_eq!(
                serde_json::from_value::<SyncEntityPayload>(payload.clone()).is_ok(),
                valid,
                "{payload}"
            );
            assert_eq!(
                crate::validate_sync_entity_payload_strict("edge", "e", false, &payload).unwrap(),
                valid,
                "{payload}"
            );
        }
    }

    #[test]
    fn chat_previous_wire_fixtures() {
        for (value, valid) in [
            (json!({"id":"c"}), true),
            (
                json!({"id":"c","kind":"chat","members":["a"],"messageCount":1.0,"lastMessageAt":1.5}),
                true,
            ),
            (
                json!({"id":"c","kind":null,"members":null,"messageCount":null}),
                true,
            ),
            (json!({"id":"c","kind":"other"}), false),
            (json!({"id":"c","messageCount":-1}), false),
            (json!({"id":"c","messageCount":1.5}), false),
            (json!({"id":"c","messageCount":9007199254740992_u64}), false),
            (json!({"id":"c","members":[1]}), false),
            (json!({"id":"c","extra":true}), false),
            (json!({}), false),
        ] {
            let payload = json!({"collection":"chat","id":"c","value":value});
            assert_eq!(
                serde_json::from_value::<SyncEntityPayload>(payload.clone()).is_ok(),
                valid,
                "{payload}"
            );
            assert_eq!(
                crate::validate_sync_entity_payload_strict("chat", "c", false, &payload).unwrap(),
                valid,
                "{payload}"
            );
        }
    }

    #[test]
    fn work_release_previous_wire_fixtures() {
        let base = json!({"id":"r","taskId":"t","rootId":"root","index":1,"value":null});
        let mut cases = vec![(base.clone(), true)];
        for (field, value, valid) in [
            ("index", json!(1.0), true),
            ("index", json!(-1), false),
            ("index", json!(1.5), false),
            ("taskId", json!(null), false),
            ("value", json!({"open":true}), true),
            ("extra", json!(true), false),
        ] {
            let mut row = base.clone();
            row[field] = value;
            cases.push((row, valid));
        }
        let mut missing = base;
        missing.as_object_mut().unwrap().remove("value");
        cases.push((missing, false));
        for (value, valid) in cases {
            let payload = json!({"collection":"workRelease","id":"r","value":value});
            assert_eq!(
                serde_json::from_value::<SyncEntityPayload>(payload.clone()).is_ok(),
                valid,
                "{payload}"
            );
            assert_eq!(
                crate::validate_sync_entity_payload_strict("workRelease", "r", false, &payload)
                    .unwrap(),
                valid,
                "{payload}"
            );
        }
    }

    #[test]
    fn migrated_payloads_use_the_serde_validator() {
        let source = include_str!("lib.rs");
        let body = source
            .split("pub fn validate_sync_entity_payload_strict(")
            .nth(1)
            .unwrap()
            .split("fn sync_entity_envelope_matches(")
            .next()
            .unwrap();
        let dispatch = "if matches!(collection, \"edge\" | \"chat\" | \"workRelease\") {";
        let owner = "serde_json::from_value::<sync_entity_payload::SyncEntityPayload>";
        let owns_payloads = |text: &str| text.contains(dispatch) && text.contains(owner);
        assert!(owns_payloads(body));
        // Negative control: removing this dispatch must make the witness fail.
        assert!(!owns_payloads(&body.replace(dispatch, "")));
    }

    #[test]
    fn edge_strict_and_stored_validation_keep_their_policies() {
        let mut stored =
            json!({"collection":"edge","id":"e","old":true,"value":{"id":"e","old":true}});
        assert!(!crate::validate_sync_entity_payload_strict("edge", "e", false, &stored).unwrap());
        assert_eq!(
            crate::normalize_sync_entity_payload("edge", "e", false, &mut stored).unwrap(),
            None
        );
        assert_eq!(
            stored,
            json!({"collection":"edge","id":"e","value":{"id":"e"}})
        );
        assert!(crate::validate_sync_entity_payload_strict("edge", "e", false, &stored).unwrap());
        stored["value"]["kind"] = json!("invalid");
        assert!(!crate::validate_sync_entity_payload_strict("edge", "e", false, &stored).unwrap());
        stored["value"] = json!({});
        assert!(crate::validate_sync_entity_payload_strict("edge", "e", true, &stored).unwrap());
    }
}
