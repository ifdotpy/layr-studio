use crate::{GenerationError, ObservableModel, UnsupportedShape, fail, identifier, quote, schema};
use std::collections::{BTreeMap, BTreeSet};
use studio_json::Value;

#[derive(Clone)]
pub(crate) enum Shape {
    Leaf,
    Frozen,
    Object(String),
    Array(Box<Shape>),
    Nullable(Box<Shape>),
    ArrayOrObject {
        array: Box<Shape>,
        object: Box<Shape>,
        dto: String,
    },
    Union {
        name: String,
        dto: String,
        tag: String,
        variants: Vec<(String, String)>,
        keyed_by: Vec<String>,
    },
}
pub(crate) struct Field {
    pub name: String,
    pub ty: String,
    pub shape: Shape,
    pub optional: bool,
    pub opaque: bool,
}
pub(crate) struct Model {
    pub name: String,
    pub dto: String,
    pub fields: Vec<Field>,
    pub identity: Option<String>,
    pub optional_identity: bool,
    pub ordered_by: Option<String>,
}
pub(crate) struct Parser<'a> {
    pub document: &'a Value,
    pub metadata: &'a [ObservableModel],
    pub models: BTreeMap<String, Model>,
    pub unions: BTreeMap<String, Shape>,
    pub roots: Vec<Shape>,
    variant_metadata: BTreeMap<String, ObservableModel>,
    visiting: BTreeSet<String>,
}
impl<'a> Parser<'a> {
    pub fn new(document: &'a Value, metadata: &'a [ObservableModel]) -> Self {
        Self {
            document,
            metadata,
            models: BTreeMap::new(),
            unions: BTreeMap::new(),
            roots: vec![],
            variant_metadata: BTreeMap::new(),
            visiting: BTreeSet::new(),
        }
    }
    pub fn parse(
        &mut self,
        value: &Value,
        name: &str,
        dto: &str,
        path: &str,
    ) -> Result<Shape, GenerationError> {
        identifier(name, path)?;
        if let Some(reference) = value["$ref"].as_str() {
            let target = reference
                .strip_prefix("#/components/schemas/")
                .ok_or_else(|| fail(path, UnsupportedShape::MissingSchema))?;
            let target_schema = schema(self.document, target)?.clone();
            if target_schema.get("$ref").is_some() {
                if !self.visiting.insert(target.into()) {
                    return Err(fail(reference, UnsupportedShape::Recursive));
                }
                let result = self.parse(&target_schema, target, dto, reference);
                self.visiting.remove(target);
                return result;
            }
            let local = self
                .metadata
                .iter()
                .any(|meta| meta.schema == target && meta.declared_fields);
            return self.parse_named(
                &target_schema,
                target,
                &if local {
                    dto.into()
                } else {
                    crate::emit::readonly_dto(target, self.metadata)
                },
                reference,
            );
        }
        self.parse_named(value, name, dto, path)
    }
    fn parse_named(
        &mut self,
        value: &Value,
        name: &str,
        dto: &str,
        path: &str,
    ) -> Result<Shape, GenerationError> {
        if value.get("allOf").is_some() {
            let merged = merge_object(self.document, value, path, &mut BTreeSet::new())?;
            return self.parse_named(&merged, name, dto, path);
        }
        // Normalize the outer wrapper on every reference, including cache hits.
        if let Some(base) = nullable_base(value) {
            return Ok(Shape::Nullable(Box::new(self.parse(
                &base,
                name,
                &format!("NonNullable<{dto}>"),
                path,
            )?)));
        }
        if let Some(union) = self.unions.get(name) {
            return Ok(union.clone());
        }
        if self.models.contains_key(name) {
            return Ok(Shape::Object(name.into()));
        }
        if self.visiting.contains(name) {
            return Err(fail(path, UnsupportedShape::Recursive));
        }
        // Constraints on already-declared fields do not introduce another model shape.
        let validation_alternatives = value["type"] == "object" && value["anyOf"].is_array();
        if validation_alternatives {
            let properties = value["properties"]
                .as_object()
                .ok_or_else(|| fail(path, UnsupportedShape::Shape))?;
            for variant in value["anyOf"].as_array().expect("checked array") {
                let constraints = variant
                    .as_object()
                    .ok_or_else(|| fail(path, UnsupportedShape::Composition))?;
                if constraints.iter().any(|(key, constraint)| {
                    key != "properties" && value.get(key) != Some(constraint)
                }) || constraints
                    .get("properties")
                    .and_then(Value::as_object)
                    .is_none_or(|fields| fields.keys().any(|key| !properties.contains_key(key)))
                {
                    return Err(fail(path, UnsupportedShape::Composition));
                }
            }
        }
        if let Some(variants) = value["anyOf"]
            .as_array()
            .or_else(|| value["oneOf"].as_array())
            .filter(|_| !validation_alternatives)
        {
            let nested = variants
                .iter()
                .map(|variant| self.is_nested(variant, &mut BTreeSet::new()))
                .collect::<Result<Vec<_>, _>>()?;
            if nested.iter().all(|nested| !nested) {
                for (index, variant) in variants.iter().enumerate() {
                    if !matches!(
                        self.parse(
                            variant,
                            &format!("{name}Scalar{index}"),
                            dto,
                            &format!("{path}/anyOf/{index}")
                        )?,
                        Shape::Leaf
                    ) {
                        return Err(fail(path, UnsupportedShape::Union));
                    }
                }
                return Ok(Shape::Leaf);
            }
            if variants.len() == 2 {
                let left = crate::metadata::resolve_variant(
                    self.document,
                    &variants[0],
                    path,
                    UnsupportedShape::Union,
                )?;
                let right = crate::metadata::resolve_variant(
                    self.document,
                    &variants[1],
                    path,
                    UnsupportedShape::Union,
                )?;
                let pair = match (left["type"].as_str(), right["type"].as_str()) {
                    (Some("array"), Some("object")) => Some((0, 1)),
                    (Some("object"), Some("array")) => Some((1, 0)),
                    _ => None,
                };
                if let Some((a, o)) = pair {
                    return Ok(Shape::ArrayOrObject {
                        dto: dto.into(),
                        array: Box::new(self.parse(
                            &variants[a],
                            &format!("{name}Array"),
                            &format!("Extract<{dto}, readonly unknown[]>"),
                            path,
                        )?),
                        object: Box::new(self.parse(
                            &variants[o],
                            &format!("{name}Object"),
                            &format!("Exclude<{dto}, readonly unknown[]>"),
                            path,
                        )?),
                    });
                }
            }
            let properties = variants
                .first()
                .and_then(|v| v["properties"].as_object())
                .ok_or_else(|| fail(path, UnsupportedShape::Union))?;
            let tag = properties
                .keys()
                .find(|key| {
                    let tags: Option<BTreeSet<_>> = variants
                        .iter()
                        .map(|v| literal(&v["properties"][*key]))
                        .collect();
                    tags.is_some_and(|tags| tags.len() == variants.len())
                        && variants.iter().all(|variant| {
                            variant["required"].as_array().is_some_and(|required| {
                                required.iter().any(|field| field.as_str() == Some(key))
                            })
                        })
                })
                .ok_or_else(|| fail(path, UnsupportedShape::Union))?
                .clone();
            self.visiting.insert(name.into());
            let mut members = vec![];
            if let Some(meta) = self.metadata.iter().find(|m| m.schema == name) {
                for field in &meta.opaque {
                    if !variants
                        .iter()
                        .any(|v| v["properties"].get(field).is_some())
                    {
                        return Err(fail(
                            &format!("{path}/properties/{field}"),
                            UnsupportedShape::UnknownField,
                        ));
                    }
                }
            }
            for (index, variant) in variants.iter().enumerate() {
                let literal = literal(&variant["properties"][&tag]).expect("checked tags");
                let variant_name = format!("{name}Variant{index}");
                if let Some(meta) = self.metadata.iter().find(|m| m.schema == name) {
                    let mut local = meta.clone();
                    local.schema = variant_name.clone();
                    local
                        .opaque
                        .retain(|field| variant["properties"].get(field).is_some());
                    self.variant_metadata.insert(variant_name.clone(), local);
                }

                self.parse(
                    variant,
                    &variant_name,
                    &format!("Extract<{dto}, {{ {tag}: {} }}>", quote(&literal)),
                    &format!(
                        "{path}/{}/{index}",
                        if value.get("anyOf").is_some() {
                            "anyOf"
                        } else {
                            "oneOf"
                        }
                    ),
                )?;
                members.push((literal, variant_name));
            }
            self.visiting.remove(name);
            let keyed_by = self
                .metadata
                .iter()
                .find(|m| m.schema == name)
                .map(|m| m.keyed_by.clone())
                .unwrap_or_default();
            for key in &keyed_by {
                for variant in variants {
                    if !identity_path(self.document, variant, &key.split('.').collect::<Vec<_>>()) {
                        return Err(fail(
                            &format!("{path}/identity/{key}"),
                            UnsupportedShape::Identity,
                        ));
                    }
                }
            }
            let union = Shape::Union {
                name: name.into(),
                dto: dto.into(),
                tag,
                variants: members,
                keyed_by,
            };
            self.unions.insert(name.into(), union.clone());
            return Ok(union);
        }
        if value["type"].is_array() {
            return Err(fail(path, UnsupportedShape::MixedTypes));
        }
        if ["allOf", "not", "if", "then", "else"]
            .iter()
            .any(|key| value.get(key).is_some())
        {
            return Err(fail(path, UnsupportedShape::Composition));
        }
        if value.get("patternProperties").is_some() {
            return Err(fail(path, UnsupportedShape::OpenObject));
        }
        if value.get("prefixItems").is_some() {
            return Err(fail(path, UnsupportedShape::Tuple));
        }
        if value.get("type").is_none()
            && value["enum"]
                .as_array()
                .is_some_and(|values| !values.is_empty() && values.iter().all(Value::is_string))
        {
            return Ok(Shape::Leaf);
        }
        match value["type"].as_str() {
            Some("string" | "number" | "integer" | "boolean" | "null") => Ok(Shape::Leaf),
            Some("array") => {
                let items = value
                    .get("items")
                    .ok_or_else(|| fail(path, UnsupportedShape::Shape))?;
                Ok(Shape::Array(Box::new(self.parse(
                    items,
                    &format!("{name}Item"),
                    &format!("({dto})[number]"),
                    &format!("{path}/items"),
                )?)))
            }
            Some("object") => {
                let owned_meta = self
                    .metadata
                    .iter()
                    .find(|m| m.schema == name)
                    .or_else(|| self.variant_metadata.get(name))
                    .cloned();
                let meta = owned_meta.as_ref();
                if value["additionalProperties"] != false
                    && !meta.is_some_and(|m| m.declared_fields)
                {
                    return Err(fail(path, UnsupportedShape::OpenObject));
                }
                let props = value["properties"]
                    .as_object()
                    .ok_or_else(|| fail(path, UnsupportedShape::Shape))?;
                if let Some(meta) = meta {
                    for field in meta
                        .identity
                        .iter()
                        .chain(meta.opaque.iter())
                        .chain(meta.omitted.iter())
                        .chain(meta.tracked_nested.iter().flatten())
                        .chain(meta.ordered_by.iter())
                    {
                        if !props.contains_key(field) {
                            return Err(fail(
                                &format!("{path}/properties/{field}"),
                                UnsupportedShape::UnknownField,
                            ));
                        }
                    }
                }
                if meta.is_some_and(|m| !m.omitted.is_empty() && !m.declared_fields) {
                    return Err(fail(path, UnsupportedShape::Shape));
                }
                self.visiting.insert(name.into());
                let mut fields = vec![];
                for (field, value) in props {
                    if meta.is_some_and(|m| m.omitted.contains(field)) {
                        continue;
                    }
                    let field_path = format!("{path}/properties/{field}");
                    identifier(&format!("field_{field}"), &field_path)?;
                    if ["constructor", "__proto__", "prototype", "apply"].contains(&field.as_str())
                    {
                        return Err(fail(&field_path, UnsupportedShape::Identifier));
                    }
                    let ty = format!("{name}Dto[{}]", quote(field));
                    let opaque = meta.is_some_and(|m| m.opaque.contains(field))
                        || meta
                            .and_then(|m| m.tracked_nested.as_ref())
                            .is_some_and(|tracked| !tracked.contains(field))
                            && self.is_nested(value, &mut BTreeSet::new())?;
                    let shape = if opaque {
                        Shape::Leaf
                    } else {
                        self.parse(
                            value,
                            &format!("{name}_{field}"),
                            &format!("NonNullable<{ty}>"),
                            &field_path,
                        )?
                    };
                    fields.push(Field {
                        name: field.clone(),
                        ty,
                        shape,
                        optional: false,
                        opaque,
                    });
                }
                let required = value["required"].as_array();
                for field in &mut fields {
                    field.optional = !required
                        .is_some_and(|names| names.iter().any(|v| v.as_str() == Some(&field.name)));
                }
                if let Some(order) = meta.and_then(|m| m.ordered_by.as_ref())
                    && (props[order]["type"] != "string"
                        || !value["required"]
                            .as_array()
                            .is_some_and(|r| r.iter().any(|v| v.as_str() == Some(order)))
                        || meta.and_then(|m| m.identity.as_ref()).is_none())
                {
                    return Err(fail(
                        &format!("{path}/properties/{order}"),
                        UnsupportedShape::Identity,
                    ));
                }
                let identity = meta.and_then(|m| m.identity.clone());
                if let Some(id) = &identity
                    && !meta.is_some_and(|m| m.optional_identity)
                    && (props[id]["type"] != "string"
                        || fields.iter().any(|f| f.name == *id && f.optional))
                {
                    return Err(fail(
                        &format!("{path}/properties/{id}"),
                        UnsupportedShape::Identity,
                    ));
                }
                if let Some(id) = &identity
                    && meta.is_some_and(|m| m.optional_identity)
                    && nullable_base(&props[id]).unwrap_or_else(|| props[id].clone())["type"]
                        != "string"
                {
                    return Err(fail(
                        &format!("{path}/properties/{id}"),
                        UnsupportedShape::Identity,
                    ));
                }
                self.visiting.remove(name);
                self.models.insert(
                    name.into(),
                    Model {
                        name: name.into(),
                        dto: dto.into(),
                        fields,
                        identity,
                        optional_identity: meta.is_some_and(|m| m.optional_identity),
                        ordered_by: meta.and_then(|m| m.ordered_by.clone()),
                    },
                );
                Ok(Shape::Object(name.into()))
            }
            _ if literal(value).is_some() => Ok(Shape::Leaf),
            _ => Err(fail(path, UnsupportedShape::Shape)),
        }
    }
    fn is_nested(
        &self,
        value: &Value,
        visiting: &mut BTreeSet<String>,
    ) -> Result<bool, GenerationError> {
        if let Some(reference) = value["$ref"].as_str() {
            if !visiting.insert(reference.into()) {
                return Err(fail(reference, UnsupportedShape::Recursive));
            }
            let name = reference
                .strip_prefix("#/components/schemas/")
                .ok_or_else(|| fail(reference, UnsupportedShape::MissingSchema))?;
            let result = self.is_nested(schema(self.document, name)?, visiting);
            visiting.remove(reference);
            return result;
        }
        if let Some(base) = nullable_base(value) {
            return self.is_nested(&base, visiting);
        }
        if matches!(value["type"].as_str(), Some("object" | "array")) {
            return Ok(true);
        }
        for key in ["oneOf", "anyOf"] {
            if let Some(variants) = value[key].as_array() {
                for variant in variants {
                    if self.is_nested(variant, visiting)? {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }
}
fn literal(value: &Value) -> Option<String> {
    value["const"]
        .as_str()
        .or_else(|| {
            value["enum"]
                .as_array()
                .filter(|v| v.len() == 1)
                .and_then(|v| v.first())
                .and_then(Value::as_str)
        })
        .map(str::to_owned)
}

fn nullable_base(value: &Value) -> Option<Value> {
    if value["nullable"] == true {
        let mut base = value.clone();
        base.as_object_mut()?.remove("nullable");
        return Some(base);
    }
    if let Some(variants) = value["anyOf"]
        .as_array()
        .or_else(|| value["oneOf"].as_array())
        && variants.len() == 2
        && variants.iter().any(|variant| variant["type"] == "null")
    {
        return variants
            .iter()
            .find(|variant| variant["type"] != "null")
            .cloned();
    }
    if let Some(types) = value["type"].as_array()
        && types.len() == 2
        && types.iter().any(|ty| ty == "null")
    {
        let mut base = value.clone();
        base["type"] = types.iter().find(|ty| **ty != "null")?.clone();
        return Some(base);
    }
    None
}

// Identity paths may traverse only required closed-object properties.
fn identity_path(document: &Value, value: &Value, path: &[&str]) -> bool {
    if let Some(reference) = value["$ref"].as_str() {
        return reference
            .strip_prefix("#/components/schemas/")
            .and_then(|name| schema(document, name).ok())
            .is_some_and(|target| identity_path(document, target, path));
    }
    if path.is_empty() {
        return value["type"] == "string";
    }
    value["required"]
        .as_array()
        .is_some_and(|fields| fields.iter().any(|f| f.as_str() == Some(path[0])))
        && identity_path(document, &value["properties"][path[0]], &path[1..])
}

/// Merge object intersections without silently choosing between conflicting owners.
fn merge_object(
    document: &Value,
    value: &Value,
    path: &str,
    visiting: &mut BTreeSet<String>,
) -> Result<Value, GenerationError> {
    let conflict = |path: &str| fail(path, UnsupportedShape::Composition);
    if let Some(reference) = value["$ref"].as_str() {
        if let Some(key) = value.as_object().and_then(|fields| {
            fields
                .keys()
                .find(|key| !matches!(key.as_str(), "$ref" | "title" | "description" | "$schema"))
        }) {
            return Err(conflict(&format!("{path}/{key}")));
        }
        if !visiting.insert(reference.into()) {
            return Err(fail(path, UnsupportedShape::Recursive));
        }
        let name = reference
            .strip_prefix("#/components/schemas/")
            .ok_or_else(|| fail(path, UnsupportedShape::MissingSchema))?;
        let result = merge_object(document, schema(document, name)?, reference, visiting);
        visiting.remove(reference);
        return result;
    }
    let mut merged = value.clone();
    let members = merged
        .as_object_mut()
        .ok_or_else(|| conflict(path))?
        .remove("allOf");
    if let Some(members) = members {
        for (index, member) in members
            .as_array()
            .ok_or_else(|| conflict(path))?
            .iter()
            .enumerate()
        {
            let member_path = format!("{path}/allOf/{index}");
            let member = merge_object(document, member, &member_path, visiting)?;
            let fields = member.as_object().ok_or_else(|| conflict(&member_path))?;
            for (key, next) in fields {
                if matches!(key.as_str(), "title" | "description" | "$schema") {
                    continue;
                }
                if key == "properties" {
                    let properties = next.as_object().ok_or_else(|| conflict(&member_path))?;
                    if merged.get(key).is_none() {
                        merged[key] = studio_json::json!({});
                    }
                    for (name, property) in properties {
                        if merged[key].get(name).is_some_and(|prior| prior != property) {
                            return Err(conflict(&format!("{member_path}/properties/{name}")));
                        }
                        merged[key][name] = property.clone();
                    }
                } else if key == "required" {
                    let required = next.as_array().ok_or_else(|| conflict(&member_path))?;
                    if merged.get(key).is_none() {
                        merged[key] = studio_json::json!([]);
                    }
                    let current = merged[key]
                        .as_array_mut()
                        .ok_or_else(|| conflict(&member_path))?;
                    for field in required {
                        if !current.contains(field) {
                            current.push(field.clone());
                        }
                    }
                } else {
                    if merged.get(key).is_some_and(|prior| prior != next) {
                        return Err(conflict(&format!("{member_path}/{key}")));
                    }
                    merged[key] = next.clone();
                }
            }
        }
    }
    if merged["type"] != "object" {
        return Err(conflict(path));
    }
    Ok(merged)
}
