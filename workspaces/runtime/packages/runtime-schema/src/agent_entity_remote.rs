//! Remote agent links and review defaults in agent wire records.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoteParentLinkDto {
    pub home: String,
    pub link: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoteWorkerLinkDto {
    pub server: String,
    pub link: String,
}

fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(deserializer)
}

fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewDefaultsDto {
    #[serde(deserialize_with = "nullable")]
    pub model: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub effort: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub fast_mode: Option<bool>,
}

pub(crate) fn schemas() -> [(&'static str, serde_json::Value); 3] {
    let mut generator = schemars::generate::SchemaSettings::openapi3()
        .for_serialize()
        .into_generator();
    let mut parent = RemoteParentLinkDto::json_schema(&mut generator).to_value();
    parent["title"] = "RemoteParentLinkDto".into();
    parent["properties"]["home"]["title"] = "Home".into();
    parent["properties"]["link"]["title"] = "Link".into();
    let mut worker = RemoteWorkerLinkDto::json_schema(&mut generator).to_value();
    worker["title"] = "RemoteWorkerLinkDto".into();
    worker["properties"]["server"]["title"] = "Server".into();
    worker["properties"]["link"]["title"] = "Link".into();
    let mut review = ReviewDefaultsDto::json_schema(&mut generator).to_value();
    review["title"] = "ReviewDefaultsDto".into();
    for (name, title) in [("model", "Model"), ("effort", "Effort")] {
        review["properties"][name] =
            serde_json::json!({"anyOf":[{"type":"null"},{"type":"string"}],"title":title});
    }
    review["properties"]["fastMode"] = serde_json::json!({"type":"boolean"});
    review["required"] = serde_json::json!(["model", "effort"]);
    [
        ("RemoteParentLinkDto", parent),
        ("RemoteWorkerLinkDto", worker),
        ("ReviewDefaultsDto", review),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn review_defaults_preserves_required_nullable_and_optional_boolean() {
        for value in [
            json!({"model":null,"effort":null}),
            json!({"model":"m","effort":"e","fastMode":false}),
        ] {
            let dto: ReviewDefaultsDto = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(dto).unwrap(), value);
        }
        for value in [
            json!({"effort":null}),
            json!({"model":null,"effort":null,"fastMode":null}),
            json!({"model":null,"effort":null,"extra":true}),
        ] {
            assert!(serde_json::from_value::<ReviewDefaultsDto>(value).is_err());
        }
    }

    #[test]
    fn published_components_match_owners() {
        let document: serde_json::Value =
            serde_json::from_str(include_str!("../../runtime-http/openapi.json")).unwrap();
        for (name, schema) in schemas() {
            assert_eq!(document["components"]["schemas"][name], schema, "{name}");
        }
    }
}
