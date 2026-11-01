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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewDefaultsDto {
    #[serde(deserialize_with = "nullable")]
    pub model: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_mode: Option<bool>,
}

fn object(
    value: &mut serde_json::Value,
) -> Result<&mut serde_json::Map<String, serde_json::Value>, String> {
    value
        .as_object_mut()
        .ok_or_else(|| "remote agent schema must be an object".into())
}

fn title(schema: &mut serde_json::Value, value: &str) -> Result<(), String> {
    object(schema)?.insert("title".into(), value.into());
    Ok(())
}

fn property<'a>(
    schema: &'a mut serde_json::Value,
    name: &str,
) -> Result<&'a mut serde_json::Value, String> {
    object(schema)?
        .get_mut("properties")
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|properties| properties.get_mut(name))
        .ok_or_else(|| format!("missing remote agent property {name}"))
}

fn normalize_nullable(value: &mut serde_json::Value, label: &str) -> Result<(), String> {
    let mut inner = value.clone();
    let obj = object(&mut inner)?;
    let types = obj
        .get("type")
        .and_then(serde_json::Value::as_array)
        .ok_or("nullable property must have a type array")?;
    if types.len() != 2 || types[1] != "null" || !types[0].is_string() {
        return Err("nullable property must have one type and null".into());
    }
    let kind = types[0].clone();
    obj.insert("type".into(), kind);
    *value = serde_json::json!({"anyOf":[{"type":"null"},inner],"title":label});
    Ok(())
}

pub(crate) fn schemas() -> Result<[(&'static str, serde_json::Value); 3], String> {
    let mut generator = schemars::generate::SchemaSettings::openapi3()
        .for_serialize()
        .into_generator();
    let mut parent = RemoteParentLinkDto::json_schema(&mut generator).to_value();
    title(&mut parent, "RemoteParentLinkDto")?;
    title(property(&mut parent, "home")?, "Home")?;
    title(property(&mut parent, "link")?, "Link")?;
    let mut worker = RemoteWorkerLinkDto::json_schema(&mut generator).to_value();
    title(&mut worker, "RemoteWorkerLinkDto")?;
    title(property(&mut worker, "server")?, "Server")?;
    title(property(&mut worker, "link")?, "Link")?;
    let mut review = ReviewDefaultsDto::json_schema(&mut generator).to_value();
    title(&mut review, "ReviewDefaultsDto")?;
    for (name, label) in [("model", "Model"), ("effort", "Effort")] {
        normalize_nullable(property(&mut review, name)?, label)?;
    }
    Ok([
        ("RemoteParentLinkDto", parent),
        ("RemoteWorkerLinkDto", worker),
        ("ReviewDefaultsDto", review),
    ])
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
        let null_fast: ReviewDefaultsDto =
            serde_json::from_value(json!({"model":null,"effort":null,"fastMode":null})).unwrap();
        assert_eq!(
            serde_json::to_value(null_fast).unwrap(),
            json!({"model":null,"effort":null})
        );
        for value in [
            json!({"effort":null}),
            json!({"model":null,"effort":null,"extra":true}),
        ] {
            assert!(serde_json::from_value::<ReviewDefaultsDto>(value).is_err());
        }
    }

    #[test]
    fn published_components_match_owners() {
        let document: serde_json::Value =
            serde_json::from_str(include_str!("../../runtime-http/openapi.json")).unwrap();
        for (name, schema) in schemas().unwrap() {
            assert_eq!(document["components"]["schemas"][name], schema, "{name}");
        }
    }

    #[test]
    fn nullable_normalization_keeps_field_constraints() {
        let mut field = json!({"type":["string","null"],"minLength":2});
        normalize_nullable(&mut field, "Model").unwrap();
        assert_eq!(
            field,
            json!({"anyOf":[{"type":"null"},{"type":"string","minLength":2}],"title":"Model"})
        );
    }
}
