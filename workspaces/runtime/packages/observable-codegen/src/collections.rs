use crate::metadata::resolve_variant;
use crate::shape::{Parser, Shape};
use crate::{
    GenerationError, ObservableResource, UnsupportedShape, fail, identifier, quote, schema,
};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use studio_json::Value;

pub(crate) fn parse(
    document: &Value,
    resource: &ObservableResource,
    parser: &mut Parser<'_>,
) -> Result<(), GenerationError> {
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
    let mut tags = BTreeSet::new();
    let mut names = BTreeSet::new();
    for collection in &resource.collections {
        identifier(&collection.facade, &path)?;
        if !tags.insert(&collection.collection) || !names.insert(&collection.facade) {
            return Err(fail(&path, UnsupportedShape::Identifier));
        }
        let mut matched = false;
        for variant in variants {
            let variant =
                resolve_variant(document, variant, &path, UnsupportedShape::MissingSchema)?;
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
        if collection.opaque {
            continue;
        }
        let shape = parser.parse(
            schema(document, &collection.schema)?,
            &collection.schema,
            &super::emit::readonly_dto(&collection.schema, parser.metadata),
            &format!("#/components/schemas/{}", collection.schema),
        )?;
        if !matches!(shape, Shape::Object(_)) {
            return Err(fail(&path, UnsupportedShape::Shape));
        }
    }
    Ok(())
}
pub(crate) fn name(resource: &ObservableResource) -> String {
    let mut chars = resource.facade.chars();
    format!(
        "{}{}Collections",
        chars.next().expect("identifier").to_ascii_uppercase(),
        chars.as_str()
    )
}
pub(crate) fn emit(
    out: &mut String,
    resource: &ObservableResource,
    metadata: &[crate::ObservableModel],
) {
    let facade = &resource.facade;
    let mut chars = facade.chars();
    let name = format!(
        "{}{}Collections",
        chars.next().expect("identifier").to_ascii_uppercase(),
        chars.as_str()
    );
    let entity = format!("{}Entity", name.trim_end_matches("Collections"));
    let projected = resource.collections.iter().any(|collection| {
        metadata
            .iter()
            .any(|m| m.schema == collection.schema && m.declared_fields)
    });
    if projected {
        writeln!(
            out,
            "export type {entity} = {};",
            resource
                .collections
                .iter()
                .map(|collection| format!(
                    "{{ readonly collection: {}; readonly id: string; readonly value: {} }}",
                    quote(&collection.collection),
                    super::emit::readonly_dto(&collection.schema, metadata)
                ))
                .collect::<Vec<_>>()
                .join(" | ")
        )
        .expect("string");
    } else {
        let tags = resource
            .collections
            .iter()
            .map(|collection| quote(&collection.collection))
            .collect::<Vec<_>>()
            .join(" | ");
        writeln!(out, "export type {entity} = DeepReadonly<Extract<components['schemas'][{}], {{ collection: {tags} }}>>;", quote(&resource.data)).expect("string");
    }
    writeln!(
        out,
        "export function serialize{name}(collections: {name}): readonly {entity}[] {{ const entities: {entity}[] = [];"
    )
    .expect("string");
    for collection in &resource.collections {
        writeln!(out, "for (const [id, value] of collections.{}) entities.push({{ collection: {}, id, value: toJS(value) }});", collection.facade, quote(&collection.collection)).expect("string");
    }
    out.push_str("return entities; }\n");
    writeln!(out, "export interface {name} {{").expect("string");
    for collection in &resource.collections {
        writeln!(
            out,
            "readonly {}: ReadonlyMap<string, {}>;",
            collection.facade,
            super::emit::readonly_dto(&collection.schema, metadata)
        )
        .expect("string");
    }
    let dto = if projected {
        format!(
            "readonly (DeepReadonly<components['schemas'][{}]> | {entity})[]",
            quote(&resource.data)
        )
    } else {
        format!(
            "DeepReadonly<components['schemas'][{}][]>",
            quote(&resource.data)
        )
    };
    writeln!(out, "}}\ntype {name}Dto = {dto};\nclass {name}Model {{").expect("string");
    for collection in &resource.collections {
        let model = if collection.opaque {
            super::emit::readonly_dto(&collection.schema, metadata)
        } else {
            format!("{}Model", collection.schema)
        };
        writeln!(
            out,
            "readonly {} = new Map<string, {}>();",
            collection.facade, model
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
        if collection.opaque {
            writeln!(out, "case {}: {{ const previous = this.{}.get(entity.id); if (!compareStructural(previous, entity.value)) this.{}.set(entity.id, freezePlain(structuredClone(entity.value))); break; }}", quote(&collection.collection), collection.facade, collection.facade).expect("string");
            continue;
        }
        writeln!(out, "case {}: {{ const previous = this.{}.get(entity.id); if (previous === undefined) this.{}.set(entity.id, new {}Model(entity.value)); else previous.apply(entity.value); break; }}", quote(&collection.collection), collection.facade, collection.facade, collection.schema).expect("string");
    }
    out.push_str("} }\n}\n}\n");
    writeln!(out, "export function create{}Mirror(): {{ readonly current: {name}; apply(dto: {name}Dto): void }} {{ const model = new {name}Model([]); return {{ get current(): {name} {{ return model; }}, apply(dto: {name}Dto): void {{ runInAction(() => {{ model.apply(dto); }}); }} }}; }}", name.trim_end_matches("Collections")).expect("string");
}
