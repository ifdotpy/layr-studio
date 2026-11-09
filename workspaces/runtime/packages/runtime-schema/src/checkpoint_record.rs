//! Typed persisted checkpoint and generated storage schema.

use crate::queue_command::Field;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use studio_json::{Number, Value};

crate::raw_json_payload!(
    #[derive(Clone, Debug, PartialEq, schemars::JsonSchema)]
    pub struct CheckpointRecord {
        fields: CheckpointFields,
        #[schemars(with = "BTreeMap<String, Value>")]
        extra
    }
);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
struct CheckpointFields {
    id: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    agent: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<Number>")]
    created: Field<Number>,
}

impl CheckpointRecord {
    pub fn from_value(value: Value) -> Result<Self, studio_json::Error> {
        studio_json::from_value(value)
    }
    pub fn id(&self) -> &String {
        &self.fields.id
    }
    pub fn agent(&self) -> Option<&str> {
        self.fields.agent.value().map(String::as_str)
    }
    pub fn into_value(self) -> Value {
        // All fields are already serializable JSON values or scalars.
        studio_json::to_value(self).expect("checkpoint fields are JSON serializable")
    }
}

pub(crate) fn schema() -> Value {
    schemars::schema_for!(CheckpointRecord).to_value()
}
pub(crate) fn check_schema_parity() -> Result<(), String> {
    let stored: Value = studio_json::from_slice(include_bytes!(
        "schemas/persisted-checkpoint-record.schema.json"
    ))
    .map_err(|error| error.to_string())?;
    if stored != schema() {
        return Err("persisted checkpoint schema must be generated from checkpoint_record::CheckpointRecord".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_json::json;

    #[test]
    fn checkpoint_schema_and_typed_record_agree() {
        check_schema_parity().unwrap();
        for (value, valid) in [
            (json!({"id":"c"}), true),
            (
                json!({"id":"c","agent":null,"created":null,"future":[1]}),
                true,
            ),
            (json!({"id":"c","agent":"a","created":1.5}), true),
            (json!({"id":"c","agent":3}), false),
            (json!({"id":"c","created":"bad"}), false),
            (json!(["c"]), false),
            (json!({}), false),
        ] {
            let decoded = studio_json::from_value::<CheckpointRecord>(value.clone());
            assert_eq!(decoded.is_ok(), valid, "{value}");
            assert_eq!(
                crate::validate("PersistedCheckpointRecord", &value).unwrap(),
                valid,
                "{value}"
            );
            if let Ok(decoded) = decoded {
                assert_eq!(decoded.into_value(), value);
            }
        }
    }
}
