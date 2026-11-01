//! Typed persisted annotations and their generated schema.
use crate::queue_command::Field;
use crate::stored_record::StoredJsonInteger;
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};

#[derive(Clone, Debug, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct AnnotationRecord(AnnotationFields);

impl<'de> Deserialize<'de> for AnnotationRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::json_object::JsonObject::<AnnotationFields>::deserialize(deserializer)
            .map(|value| Self(value.0))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AnnotationFields {
    id: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    agent: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    path: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    text: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "line_schema")]
    line: Field<StoredJsonInteger>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<Number>")]
    created: Field<Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    root_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    turn_id: Field<String>,
}

fn line_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"type":"integer","x-safe-integer":"unsigned"},{"type":"null"}]})
}

pub fn valid_line(value: &Value) -> bool {
    value.is_null() || StoredJsonInteger::deserialize(value).is_ok()
}

impl AnnotationRecord {
    pub fn from_value(value: Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
    }
    pub fn id(&self) -> &String {
        &self.0.id
    }
    pub fn sync_view(&self, bound: &impl Fn(&str, &str) -> String) -> Self {
        let mut view = self.clone();
        view.0.id = bound(&view.0.id, "id");
        for (field, name) in [
            (&mut view.0.agent, "agent"),
            (&mut view.0.root_id, "rootId"),
            (&mut view.0.path, "path"),
            (&mut view.0.text, "text"),
            (&mut view.0.turn_id, "turnId"),
        ] {
            if let Field::Value(value) = field {
                *value = bound(value, name);
            }
        }
        view
    }
    pub fn value(&self) -> Value {
        // Every field is already serializable JSON data.
        serde_json::to_value(self).expect("annotation fields are JSON serializable")
    }
    pub fn into_value(self) -> Value {
        self.value()
    }
}

pub(crate) fn schema() -> Value {
    schemars::schema_for!(AnnotationRecord).to_value()
}
pub(crate) fn check_schema_parity() -> Result<(), String> {
    let stored: Value = serde_json::from_slice(include_bytes!(
        "schemas/persisted-annotation-record.schema.json"
    ))
    .map_err(|error| error.to_string())?;
    if stored != schema() {
        return Err("persisted annotation schema must be generated from annotation_record::AnnotationRecord".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn schema_and_typed_annotations_accept_the_same_shapes() {
        check_schema_parity().unwrap();
        for (value, valid) in [
            (json!({"id":"a"}), true),
            (
                json!({"id":"a","line":-1.0,"created":null,"extra":[1]}),
                false,
            ),
            (json!({"id":"a","line":u64::MAX}), true),
            (json!({"id":"a","line":1.5}), false),
            (json!({"id":"a","created":"bad"}), false),
            (json!(["a"]), false),
        ] {
            assert_eq!(
                serde_json::from_value::<AnnotationRecord>(value.clone()).is_ok(),
                valid,
                "{value}"
            );
            assert_eq!(
                crate::validate("PersistedAnnotationRecord", &value).unwrap(),
                valid,
                "{value}"
            );
        }
    }
}
