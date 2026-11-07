use crate::emit::{HEADER, emit_model, emit_union};
use crate::shape::{Parser, Shape};
use crate::{
    GenerationError, ObservableModel, ObservableResource, UnsupportedShape, fail, identifier,
    quote, schema,
};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use studio_json::Value;

/// Selected keyed views share the singleton resource's existing interest and status.
pub(crate) fn render(
    document: &Value,
    metadata: &[ObservableModel],
    resources: &[ObservableResource],
) -> Result<String, GenerationError> {
    let [resource] = resources else {
        return Err(fail("#/resources", UnsupportedShape::Resource));
    };
    let path = format!("#/resources/{}", resource.kind);
    if !resource.args.is_empty() || resource.collections.is_empty() {
        return Err(fail(&path, UnsupportedShape::Resource));
    }
    crate::metadata::validate(document, resource)?;
    identifier(&resource.facade, &path)?;
    let payload = schema(document, &resource.data)?;
    let variants = payload["oneOf"]
        .as_array()
        .ok_or_else(|| fail(&path, UnsupportedShape::Union))?;
    let mut parser = Parser::new(document, metadata);
    let mut tags = BTreeSet::new();
    let mut names = BTreeSet::new();
    for collection in &resource.collections {
        identifier(&collection.facade, &path)?;
        if !tags.insert(&collection.collection) || !names.insert(&collection.facade) {
            return Err(fail(&path, UnsupportedShape::Identifier));
        }
        let mut matched = false;
        for variant in variants {
            let variant = if let Some(reference) = variant["$ref"].as_str() {
                schema(
                    document,
                    reference
                        .strip_prefix("#/components/schemas/")
                        .ok_or_else(|| fail(&path, UnsupportedShape::MissingSchema))?,
                )?
            } else {
                variant
            };
            let properties = &variant["properties"];
            if properties["collection"]["const"] != collection.collection {
                continue;
            }
            let required = variant["required"]
                .as_array()
                .ok_or_else(|| fail(&path, UnsupportedShape::Resource))?;
            if properties["id"]["type"] != "string"
                || properties["value"]["$ref"]
                    != format!("#/components/schemas/{}", collection.schema)
                || ["collection", "id", "value"]
                    .iter()
                    .any(|name| !required.iter().any(|value| value.as_str() == Some(name)))
            {
                return Err(fail(&path, UnsupportedShape::Resource));
            }
            matched = true;
        }
        if !matched {
            return Err(fail(&path, UnsupportedShape::Resource));
        }
        let shape = parser.parse(
            schema(document, &collection.schema)?,
            &collection.schema,
            &format!(
                "DeepReadonly<components['schemas'][{}]>",
                quote(&collection.schema)
            ),
            &format!("#/components/schemas/{}", collection.schema),
        )?;
        if !matches!(shape, Shape::Object(_)) {
            return Err(fail(&path, UnsupportedShape::Shape));
        }
    }
    for model in metadata {
        if !parser.models.contains_key(&model.schema) {
            return Err(fail(
                &format!("#/components/schemas/{}", model.schema),
                UnsupportedShape::MissingSchema,
            ));
        }
    }
    let mut out = String::from(HEADER);
    for model in parser.models.values() {
        emit_model(&mut out, model, &parser);
    }
    for union in parser.unions.values() {
        emit_union(&mut out, union);
    }
    let facade = &resource.facade;
    let mut chars = facade.chars();
    let name = format!(
        "{}{}Collections",
        chars.next().expect("identifier").to_ascii_uppercase(),
        chars.as_str()
    );
    writeln!(out, "export interface {name} {{").expect("string");
    for collection in &resource.collections {
        writeln!(
            out,
            "readonly {}: ReadonlyMap<string, DeepReadonly<components['schemas'][{}]>>;",
            collection.facade,
            quote(&collection.schema)
        )
        .expect("string");
    }
    writeln!(
        out,
        "}}\ntype {name}Dto = DeepReadonly<components['schemas'][{}][]>;\nclass {name}Model {{",
        quote(&resource.data)
    )
    .expect("string");
    for collection in &resource.collections {
        writeln!(
            out,
            "readonly {} = new Map<string, {}Model>();",
            collection.facade, collection.schema
        )
        .expect("string");
    }
    writeln!(
        out,
        "constructor(dto: {name}Dto) {{\nmakeObservable(this, {{"
    )
    .expect("string");
    for collection in &resource.collections {
        writeln!(out, "{}: observableShallow,", collection.facade).expect("string");
    }
    out.push_str("apply: action,\n});\nthis.apply(dto);\n}\n");
    writeln!(out, "apply(dto: {name}Dto): void {{").expect("string");
    for collection in &resource.collections {
        writeln!(out, "const {}Ids = new Set<string>();", collection.facade).expect("string");
    }
    out.push_str("for (const entity of dto) { switch (entity.collection) {\n");
    for collection in &resource.collections {
        writeln!(
            out,
            "case {}: if ({}Ids.has(entity.id)) throw new Error({}); {}Ids.add(entity.id); break;",
            quote(&collection.collection),
            collection.facade,
            quote(&format!("Duplicate identity in {}", collection.collection)),
            collection.facade
        )
        .expect("string");
    }
    out.push_str("} }\n");
    for collection in &resource.collections {
        writeln!(
            out,
            "for (const id of this.{}.keys()) {{ if (!{}Ids.has(id)) this.{}.delete(id); }}",
            collection.facade, collection.facade, collection.facade
        )
        .expect("string");
    }
    out.push_str("for (const entity of dto) { switch (entity.collection) {\n");
    for collection in &resource.collections {
        writeln!(out, "case {}: {{ const previous = this.{}.get(entity.id); if (previous === undefined) this.{}.set(entity.id, new {}Model(entity.value)); else previous.apply(entity.value); break; }}", quote(&collection.collection), collection.facade, collection.facade, collection.schema).expect("string");
    }
    out.push_str("} }\n}\n}\n");
    writeln!(out, "export function create{}Mirror(): {{ readonly current: {name}; apply(dto: {name}Dto): void }} {{ const model = new {name}Model([]); return {{ get current(): {name} {{ return model; }}, apply(dto: {name}Dto): void {{ runInAction(() => {{ model.apply(dto); }}); }} }}; }}", name.trim_end_matches("Collections")).expect("string");
    writeln!(out, "export interface ObservableResources {{ {facade}(): ResourceSlot<{name}>; }}\nexport function createObservableResources(runtime: ObservableResourceRuntime): ObservableResources {{\nlet slot: ResourceSlot<{name}> | undefined;\nreturn {{ {facade}() {{\nif (slot !== undefined) return slot;\nconst ref: Extract<ResourceRef, {{kind: {}}}> = {{kind: {}}};\nreturn slot = runtime.slot({{ key: subscriptionKey(ref), ref, create: (dto) => new {name}Model(dto), apply: (data, dto) => {{ data.apply(dto); }} }});\n}} }};\n}}", quote(&resource.kind), quote(&resource.kind)).expect("string");
    Ok(out)
}
