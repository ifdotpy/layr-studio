use crate::{GenerationError, ObservableResource, UnsupportedShape, fail, schema};
use studio_json::Value;

pub(crate) fn resolve_variant<'a>(
    document: &'a Value,
    variant: &'a Value,
    path: &str,
    unsupported: UnsupportedShape,
) -> Result<&'a Value, GenerationError> {
    match variant["$ref"].as_str() {
        Some(reference) => schema(
            document,
            reference
                .strip_prefix("#/components/schemas/")
                .ok_or_else(|| fail(path, unsupported))?,
        ),
        None => Ok(variant),
    }
}

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
    if data.is_null()
        || !resource.mapped && reference != &format!("#/components/schemas/{}", resource.data)
    {
        return Err(fail(&path, UnsupportedShape::Resource));
    }
    let reference = schema(document, "ResourceRef")?;
    let variants = reference["oneOf"]
        .as_array()
        .ok_or_else(|| fail(&path, UnsupportedShape::Resource))?;
    for variant in variants {
        let variant = resolve_variant(document, variant, &path, UnsupportedShape::Resource)?;
        let kind = &variant["properties"]["kind"];
        if kind["const"] != resource.kind && kind["enum"] != studio_json::json!([resource.kind]) {
            continue;
        }
        let fields = variant["properties"]
            .as_object()
            .ok_or_else(|| fail(&path, UnsupportedShape::Resource))?;
        let _required = variant["required"]
            .as_array()
            .ok_or_else(|| fail(&path, UnsupportedShape::Resource))?;
        if fields.len() != resource.args.len() + 1 {
            return Err(fail(&path, UnsupportedShape::Resource));
        }
        for arg in &resource.args {
            if fields.get(&arg.name).is_none_or(|field| {
                !matches!(
                    field["type"].as_str(),
                    Some("string" | "integer" | "number" | "boolean")
                )
            }) {
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

/// Argument representation and omission come from the ResourceRef schema owner.
pub(crate) fn argument(
    document: &Value,
    resource: &ObservableResource,
    name: &str,
) -> Result<(String, bool), GenerationError> {
    let path = format!("#/resources/{}/args/{name}", resource.kind);
    let variants = schema(document, "ResourceRef")?["oneOf"]
        .as_array()
        .ok_or_else(|| fail(&path, UnsupportedShape::Resource))?;
    for variant in variants {
        let variant = resolve_variant(document, variant, &path, UnsupportedShape::Resource)?;
        let kind = &variant["properties"]["kind"];
        if kind["const"] != resource.kind && kind["enum"] != studio_json::json!([resource.kind]) {
            continue;
        }
        let ty = match variant["properties"][name]["type"].as_str() {
            Some("string") => "string",
            Some("number" | "integer") => "number",
            Some("boolean") => "boolean",
            _ => return Err(fail(&path, UnsupportedShape::Resource)),
        };
        let optional = !variant["required"]
            .as_array()
            .is_some_and(|required| required.iter().any(|v| v == name));
        return Ok((ty.into(), optional));
    }
    Err(fail(&path, UnsupportedShape::Resource))
}
