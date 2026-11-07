use crate::{GenerationError, ObservableResource, UnsupportedShape, fail, schema};
use studio_json::Value;

/// Metadata must refer to the existing transport contract, rather than invent a registry.
pub(crate) fn validate(
    document: &Value,
    resource: &ObservableResource,
) -> Result<(), GenerationError> {
    let path = format!("#/resources/{}", resource.kind);
    let data = schema(document, "SubscriptionDataMap")?;
    let data = &data["properties"][&resource.kind];
    let reference = if resource.collections.is_empty() {
        &data["$ref"]
    } else {
        if data["type"] != "array" {
            return Err(fail(&path, UnsupportedShape::Resource));
        }
        &data["items"]["$ref"]
    };
    if reference != &format!("#/components/schemas/{}", resource.data) {
        return Err(fail(&path, UnsupportedShape::Resource));
    }
    let reference = schema(document, "ResourceRef")?;
    let variants = reference["oneOf"]
        .as_array()
        .ok_or_else(|| fail(&path, UnsupportedShape::Resource))?;
    for variant in variants {
        let variant = match variant["$ref"].as_str() {
            Some(reference) => schema(
                document,
                reference
                    .strip_prefix("#/components/schemas/")
                    .ok_or_else(|| fail(&path, UnsupportedShape::Resource))?,
            )?,
            None => variant,
        };
        let kind = &variant["properties"]["kind"];
        if kind["const"] != resource.kind && kind["enum"] != studio_json::json!([resource.kind]) {
            continue;
        }
        let fields = variant["properties"]
            .as_object()
            .ok_or_else(|| fail(&path, UnsupportedShape::Resource))?;
        let required = variant["required"]
            .as_array()
            .ok_or_else(|| fail(&path, UnsupportedShape::Resource))?;
        if fields.len() != resource.args.len() + 1 || required.len() != fields.len() {
            return Err(fail(&path, UnsupportedShape::Resource));
        }
        for arg in &resource.args {
            if fields
                .get(&arg.name)
                .is_none_or(|field| field["type"] != "string")
                || !required.iter().any(|field| field == &arg.name)
            {
                return Err(fail(
                    &format!("{path}/args/{}", arg.name),
                    UnsupportedShape::Resource,
                ));
            }
        }
        return Ok(());
    }
    Err(fail(&path, UnsupportedShape::Resource))
}
