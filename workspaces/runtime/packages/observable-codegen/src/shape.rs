use crate::{GenerationError, ObservableModel, UnsupportedShape, fail, identifier, quote, schema};
use std::collections::{BTreeMap, BTreeSet};
use studio_json::Value;

#[derive(Clone)]
pub(crate) enum Shape {
    Leaf,
    Object(String),
    Array(Box<Shape>),
    Nullable(Box<Shape>),
    Union {
        name: String,
        dto: String,
        tag: String,
        variants: Vec<(String, String)>,
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
    pub ordered_by: Option<String>,
}
pub(crate) struct Parser<'a> {
    pub document: &'a Value,
    pub metadata: &'a [ObservableModel],
    pub models: BTreeMap<String, Model>,
    pub unions: BTreeMap<String, Shape>,
    visiting: BTreeSet<String>,
}
impl<'a> Parser<'a> {
    pub fn new(document: &'a Value, metadata: &'a [ObservableModel]) -> Self {
        Self {
            document,
            metadata,
            models: BTreeMap::new(),
            unions: BTreeMap::new(),
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
            return self.parse_named(
                &target_schema,
                target,
                &format!("DeepReadonly<components['schemas'][{}]>", quote(target)),
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
        if let Some(union) = self.unions.get(name) {
            return Ok(union.clone());
        }
        if self.models.contains_key(name) {
            return Ok(Shape::Object(name.into()));
        }
        if self.visiting.contains(name) {
            return Err(fail(path, UnsupportedShape::Recursive));
        }
        // OpenAPI nullable and schemars anyOf null both preserve explicit null.
        if value["nullable"] == true {
            let mut base = value.clone();
            base.as_object_mut()
                .expect("schema object")
                .remove("nullable");
            return Ok(Shape::Nullable(Box::new(self.parse(
                &base,
                name,
                &format!("NonNullable<{dto}>"),
                path,
            )?)));
        }
        if let Some(variants) = value["anyOf"]
            .as_array()
            .or_else(|| value["oneOf"].as_array())
        {
            if variants.len() == 2 && variants.iter().any(|v| v["type"] == "null") {
                let base = variants
                    .iter()
                    .find(|v| v["type"] != "null")
                    .expect("non-null member");
                return Ok(Shape::Nullable(Box::new(self.parse(
                    base,
                    name,
                    &format!("NonNullable<{dto}>"),
                    path,
                )?)));
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
            for (index, variant) in variants.iter().enumerate() {
                let literal = literal(&variant["properties"][&tag]).expect("checked tags");
                let variant_name = format!("{name}Variant{index}");
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
            let union = Shape::Union {
                name: name.into(),
                dto: dto.into(),
                tag,
                variants: members,
            };
            self.unions.insert(name.into(), union.clone());
            return Ok(union);
        }
        if let Some(types) = value["type"].as_array() {
            if types.len() == 2 && types.iter().any(|v| v == "null") {
                let mut base = value.clone();
                base["type"] = types.iter().find(|v| **v != "null").expect("type").clone();
                return Ok(Shape::Nullable(Box::new(self.parse(
                    &base,
                    name,
                    &format!("NonNullable<{dto}>"),
                    path,
                )?)));
            }
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
                if value["additionalProperties"] != false {
                    return Err(fail(path, UnsupportedShape::OpenObject));
                }
                let props = value["properties"]
                    .as_object()
                    .ok_or_else(|| fail(path, UnsupportedShape::Shape))?;
                let meta = self.metadata.iter().find(|m| m.schema == name);
                if let Some(meta) = meta {
                    for field in meta
                        .identity
                        .iter()
                        .chain(meta.opaque.iter())
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
                self.visiting.insert(name.into());
                let mut fields = vec![];
                for (field, value) in props {
                    let field_path = format!("{path}/properties/{field}");
                    identifier(field, &field_path)?;
                    let ty = format!("{name}Dto[{}]", quote(field));
                    let opaque = meta.is_some_and(|m| m.opaque.contains(field));
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
                    && (props[id]["type"] != "string"
                        || fields.iter().any(|f| f.name == *id && f.optional))
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
                        ordered_by: meta.and_then(|m| m.ordered_by.clone()),
                    },
                );
                Ok(Shape::Object(name.into()))
            }
            _ if literal(value).is_some() => Ok(Shape::Leaf),
            _ => Err(fail(path, UnsupportedShape::Shape)),
        }
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
