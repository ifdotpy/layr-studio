//! Serde owners for migrated sync entity payload variants.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "collection", rename_all = "camelCase", deny_unknown_fields)]
pub enum SyncEntityPayload {
    Edge { id: String, value: EdgeEntity },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "EdgeEntityDto")]
pub struct EdgeEntity {
    pub id: String,
    #[serde(default)]
    pub kind: Option<EdgeKind>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
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
    let edge = schema["oneOf"][0]
        .as_object()
        .ok_or("missing edge variant")?;
    schemas.insert("SyncEdgeEntityPayload".into(), edge.clone().into());
    Ok(schemas)
}

#[cfg(test)]
mod tests {
    use super::SyncEntityPayload;
    use serde_json::json;

    #[test]
    fn edge_serde_matches_existing_schema() {
        for value in [
            json!({"collection":"edge","id":"e","value":{"id":"e"}}),
            json!({"collection":"edge","id":"e","value":{"id":"e","kind":"chat","source":null,"target":"t"}}),
            json!({"collection":"edge","id":"e","value":{"id":"e","kind":"unknown"}}),
            json!({"collection":"edge","id":"e","value":{"id":"e","unexpected":true}}),
            json!({"collection":"edge","id":"e","value":{}}),
        ] {
            assert_eq!(
                serde_json::from_value::<SyncEntityPayload>(value.clone()).is_ok(),
                crate::first_error("SyncEdgeEntityPayload", &value)
                    .unwrap()
                    .is_none(),
                "{value}"
            );
        }
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
