use super::*;
use studio_json::json;

fn fixture() -> Value {
    json!({"components":{"schemas":{
        "Row":{"type":"object","additionalProperties":false,"required":["id","sortKey","title","nested","tags","choice","opaque","maybe","nil","a","b","c","d","e","f"],"properties":{
            "id":{"type":"string"},"sortKey":{"type":"string"},"title":{"type":["string","null"]},
            "nested":{"type":"object","additionalProperties":false,"required":["count"],"properties":{"count":{"type":"number"}}},
            "tags":{"type":"array","items":{"type":"string"}},
            "optional":{"type":"string"},
            "optionalTags":{"anyOf":[{"type":"array","items":{"type":"string"}},{"type":"null"}]},
            "nil":{"type":"null"},
            "a":{"$ref":"#/components/schemas/Maybe"},"b":{"$ref":"#/components/schemas/Maybe"},
            "c":{"$ref":"#/components/schemas/FlagMaybe"},"d":{"$ref":"#/components/schemas/FlagMaybe"},
            "e":{"$ref":"#/components/schemas/TypeMaybe"},"f":{"$ref":"#/components/schemas/TypeMaybe"},
            "maybe":{"anyOf":[{"type":"object","additionalProperties":false,"required":["value"],"properties":{"value":{"type":"string"}}},{"type":"null"}]},
            "opaque":{"anyOf":[{"type":"object","additionalProperties":true},{"type":"null"}]},
            "choice":{"oneOf":[
                {"type":"object","additionalProperties":false,"required":["kind","count"],"properties":{"kind":{"type":"string","enum":["count"]},"count":{"type":"integer"}}},
                {"type":"object","additionalProperties":false,"required":["kind","text"],"properties":{"kind":{"type":"string","enum":["text"]},"text":{"type":"string"}}}
            ]}
        }},
        "Maybe":{"anyOf":[{"type":"object","additionalProperties":false,"required":["value"],"properties":{"value":{"type":"string"}}},{"type":"null"}]},
        "FlagMaybe":{"type":"object","nullable":true,"additionalProperties":false,"required":["value"],"properties":{"value":{"type":"string"}}},
        "TypeMaybe":{"type":["object","null"],"additionalProperties":false,"required":["value"],"properties":{"value":{"type":"string"}}},
        "List":{"type":"object","additionalProperties":false,"required":["rows","plain"],"properties":{
            "rows":{"type":"array","items":{"$ref":"#/components/schemas/Row"}},
            "plain":{"type":"array","items":{"type":"number"}}
        }},
        "ResourceRef":{"oneOf":[{"type":"object","additionalProperties":false,"required":["kind","agentId","teamId"],"properties":{"kind":{"type":"string","enum":["fixture"]},"agentId":{"type":"string"},"teamId":{"type":"string"}}}]},
        "SubscriptionDataMap":{"type":"object","required":["fixture"],"properties":{"fixture":{"$ref":"#/components/schemas/List"}}}
    }}})
}
fn models() -> Vec<ObservableModel> {
    vec![ObservableModel {
        schema: "Row".into(),
        identity: Some("id".into()),
        optional_identity: false,
        omitted: vec![],
        keyed_by: vec![],
        ordered_by: Some("sortKey".into()),
        opaque: vec!["opaque".into()],
        tracked_nested: None,
        declared_fields: false,
        local_fields: false,
    }]
}

#[test]
fn selected_collections_use_the_existing_array_resource() {
    let mut document = fixture();
    document["components"]["schemas"]["ResourceRef"] = json!({"oneOf":[{
        "type":"object","additionalProperties":false,"required":["kind"],
        "properties":{"kind":{"type":"string","const":"state"}}
    }]});
    document["components"]["schemas"]["Tail"] = json!({"type":"object","additionalProperties":false,"required":["tail"],"properties":{"tail":{"type":"string"}}});
    document["components"]["schemas"]["Payload"] = json!({"oneOf":[
        {"type":"object","additionalProperties":false,"required":["collection","id","value"],"properties":{
            "collection":{"type":"string","const":"row"},"id":{"type":"string"},"value":{"$ref":"#/components/schemas/Row"}
        }},
        {"type":"object","additionalProperties":false,"required":["collection","id","value"],"properties":{
            "collection":{"type":"string","const":"tail"},"id":{"type":"string"},"value":{"$ref":"#/components/schemas/Tail"}
        }},
        {"type":"object","additionalProperties":false,"required":["collection","id","value"],"properties":{
            "collection":{"type":"string","const":"ignored"},"id":{"type":"string"},"value":{"type":"object","additionalProperties":true}
        }}
    ]});
    document["components"]["schemas"]["SubscriptionDataMap"] = json!({
        "type":"object","required":["state"],"properties":{"state":{"type":"array","items":{"$ref":"#/components/schemas/Payload"}}}
    });
    let resources = vec![ObservableResource::collections(
        "state",
        "state",
        "Payload",
        vec![
            ObservableCollection::named("row", "rows", "Row"),
            ObservableCollection::named("tail", "tails", "Tail"),
        ],
    )];
    let generated =
        render(&document, &models(), &resources).expect("selected state collections render");
    assert!(generated.contains("state(): ResourceSlot<StateCollections>"));
    assert!(generated.contains("export function serializeStateCollections("));
    assert!(generated.contains("export type StateEntity ="));
    assert!(!generated.contains("ignoredModel"));
    assert!(
        !generated.contains("function reconcileRow("),
        "map-only models need no array helper"
    );
    assert_witness(&document, &generated, true);
    let mut bad = resources.clone();
    bad[0].collections[0].schema = "List".into();
    assert_eq!(
        render(&document, &models(), &bad)
            .expect_err("mismatched selected value")
            .kind,
        UnsupportedShape::Resource
    );
    bad = resources;
    let duplicate = bad[0].collections[0].clone();
    bad[0].collections.push(duplicate);
    assert_eq!(
        render(&document, &models(), &bad)
            .expect_err("duplicate facade")
            .kind,
        UnsupportedShape::Identifier
    );
}

#[test]
fn object_validation_alternatives_do_not_create_union_models() {
    let mut document = fixture();
    document["components"]["schemas"]["Row"]["properties"]["nested"]["anyOf"] =
        json!([{"properties":{"count":{"minimum":1}}}]);
    render(&document, &models(), &resources()).expect("validation narrows existing fields");
    let nested = &mut document["components"]["schemas"]["Row"]["properties"]["nested"];
    let mut complete = nested.clone();
    complete.as_object_mut().expect("object").remove("anyOf");
    complete["properties"]["count"]["minimum"] = json!(1);
    nested["anyOf"] = json!([complete]);
    render(&document, &models(), &resources())
        .expect("complete validation alternative repeats the declared shape");
    document["components"]["schemas"]["Row"]["properties"]["nested"]["anyOf"] =
        json!([{"properties":{"extra":{"type":"string"}}}]);
    assert!(
        render(&document, &models(), &resources()).is_err(),
        "alternatives cannot add fields"
    );
}

#[test]
fn component_aliases_resolve_and_cycles_fail_with_the_schema_path() {
    let mut document = fixture();
    document["components"]["schemas"]["Row"]["properties"]["title"] =
        json!({"$ref":"#/components/schemas/Alias"});
    document["components"]["schemas"]["Alias"] = json!({"$ref":"#/components/schemas/Scalar"});
    document["components"]["schemas"]["Scalar"] = json!({"type":"string"});
    render(&document, &models(), &resources()).expect("named scalar alias");
    document["components"]["schemas"]["Scalar"] = json!({"$ref":"#/components/schemas/Alias"});
    assert_eq!(
        render(&document, &models(), &resources())
            .expect_err("alias cycle")
            .kind,
        UnsupportedShape::Recursive
    );
}
fn resources() -> Vec<ObservableResource> {
    vec![ObservableResource {
        kind: "fixture".into(),
        facade: "fixture".into(),
        data: "List".into(),
        args: vec![
            ObservableArg {
                name: "agentId".into(),
                brand: "AgentId".into(),
            },
            ObservableArg {
                name: "teamId".into(),
                brand: "TeamId".into(),
            },
        ],
        collections: vec![],
        mapped: false,
        mirror_only: false,
        opaque: false,
    }]
}

#[test]
fn deterministic_fixture_and_typed_path_failures() {
    let document = fixture();
    let generated = render(&document, &models(), &resources()).expect("fixture renders");
    assert!(
        generated.contains("function reconcileRow("),
        "identified array fields retain their helper"
    );
    assert_eq!(
        generated,
        render(&document, &models(), &resources()).expect("repeat")
    );
    assert!(!generated.contains("any"));
    assert!(!generated.contains("unknown"));
    assert!(!generated.contains(" as "));
    for field in ["id", "opaque", "sortKey"] {
        let mut bad = models();
        match field {
            "id" => bad[0].identity = Some("missing".into()),
            "opaque" => bad[0].opaque = vec!["missing".into()],
            "sortKey" => bad[0].ordered_by = Some("missing".into()),
            _ => unreachable!(),
        }
        let error = render(&document, &bad, &resources()).expect_err("bad metadata");
        assert_eq!(error.path, "#/components/schemas/Row/properties/missing");
        assert_eq!(error.kind, UnsupportedShape::UnknownField);
    }
    for (invalid, kind) in [
        (json!({}), UnsupportedShape::Shape),
        (
            json!({"type":"object","additionalProperties":true}),
            UnsupportedShape::OpenObject,
        ),
        (
            json!({"allOf":[{"type":"string"}]}),
            UnsupportedShape::Composition,
        ),
        (
            json!({"type":["string","number"]}),
            UnsupportedShape::MixedTypes,
        ),
        (
            json!({"type":"array","prefixItems":[{"type":"string"}]}),
            UnsupportedShape::Tuple,
        ),
    ] {
        let mut bad = document.clone();
        bad["components"]["schemas"]["Row"]["properties"]["nested"] = invalid;
        let error = render(&bad, &models(), &resources()).expect_err("unsupported");
        assert!(error.path.contains("/Row/properties/nested"), "{error}");
        assert_eq!(error.kind, kind);
    }
    let mut recursive = document.clone();
    recursive["components"]["schemas"]["Row"]["properties"]["nested"] =
        json!({"$ref":"#/components/schemas/Row"});
    assert_eq!(
        render(&recursive, &models(), &resources())
            .expect_err("recursive")
            .kind,
        UnsupportedShape::Recursive
    );
    let mut bad_union = document.clone();
    bad_union["components"]["schemas"]["Row"]["properties"]["choice"]["oneOf"][1]["properties"]["kind"]
        ["enum"] = json!(["count"]);
    assert_eq!(
        render(&bad_union, &models(), &resources())
            .expect_err("duplicate tags")
            .kind,
        UnsupportedShape::Union
    );
    let mut bad_identity = document.clone();
    bad_identity["components"]["schemas"]["Row"]["properties"]["id"] = json!({"type":"number"});
    assert_eq!(
        render(&bad_identity, &models(), &resources())
            .expect_err("identity type")
            .kind,
        UnsupportedShape::Identity
    );
    let mut bad_resource = resources();
    bad_resource[0].data = "Row".into();
    let error = render(&document, &models(), &bad_resource).expect_err("mismatched resource DTO");
    assert_eq!(error.kind, UnsupportedShape::Resource);
    assert_eq!(error.path, "#/resources/fixture");
    let mut bad_argument = resources();
    bad_argument[0].args[0].name = "missing".into();
    assert_eq!(
        render(&document, &models(), &bad_argument)
            .expect_err("argument")
            .kind,
        UnsupportedShape::Resource
    );
    let mut bad_field = document;
    bad_field["components"]["schemas"]["Row"]["properties"]["apply"] = json!({"type":"string"});
    let error = render(&bad_field, &models(), &resources()).expect_err("method collision");
    assert_eq!(error.kind, UnsupportedShape::Identifier);
    assert_eq!(error.path, "#/components/schemas/Row/properties/apply");
}

#[test]
fn generated_fixture_compiles_and_updates_through_facade() {
    let document = fixture();
    let generated = render(&document, &models(), &resources()).expect("fixture");
    assert_witness(&document, &generated, false);
}

fn assert_witness(document: &Value, generated: &str, collections: bool) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let root = std::env::current_dir().expect("cwd").join("../../../..");
    let mut child =
        Command::new("node")
            .arg(root.join(
                "workspaces/tooling/apps/repository-checks/tests/observable-codegen-witness.ts",
            ))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("node");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(
            studio_json::to_string(
                &json!({"source":generated,"document":document,"collections":collections}),
            )
            .expect("json")
            .as_bytes(),
        )
        .expect("write");
    let output = child.wait_with_output().expect("witness exits");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn singleton_returns_cached_slot_before_computing_the_canonical_key() {
    let mut document = fixture();
    document["components"]["schemas"]["ResourceRef"]["oneOf"][0] = json!({
        "type":"object", "additionalProperties":false,
        "properties":{"kind":{"type":"string","const":"fixture"}}, "required":["kind"]
    });
    let mut resources = resources();
    resources[0].args.clear();
    let source = render(&document, &models(), &resources).expect("singleton");
    let cached = source
        .find("if (fixtureSlot !== undefined) return fixtureSlot;")
        .expect("cached return");
    let key = source
        .find("const key = subscriptionKey(ref)")
        .expect("canonical key");
    assert!(cached < key);
}

#[test]
fn repeated_named_nullable_references_preserve_the_outer_shape() {
    for name in ["Maybe", "FlagMaybe", "TypeMaybe"] {
        let document = fixture();
        let mut parser = crate::shape::Parser::new(&document, &[]);
        let reference = json!({"$ref":format!("#/components/schemas/{name}")});
        for _ in 0..2 {
            let parsed = parser
                .parse(&reference, name, "Dto", "#/test")
                .expect("nullable component");
            assert!(
                matches!(parsed, crate::shape::Shape::Nullable(_)),
                "{name} lost outer nullability"
            );
        }
    }
}

#[test]
fn resource_arguments_reject_generated_binding_collisions() {
    for name in [
        "key",
        "ref",
        "runtime",
        "cached",
        "slot",
        "cache0",
        "fixtureSlots",
        "subscriptionKey",
        "Map",
        "ListModel",
    ] {
        let mut document = fixture();
        document["components"]["schemas"]["ResourceRef"]["oneOf"][0] = json!({
            "type":"object","additionalProperties":false,
            "properties":{"kind":{"type":"string","const":"fixture"},name:{"type":"string"}},
            "required":["kind",name]
        });
        let mut resources = resources();
        resources[0].args = vec![ObservableArg {
            name: name.into(),
            brand: "AgentId".into(),
        }];
        let error =
            render(&document, &models(), &resources).expect_err("generated binding collision");
        assert_eq!(error.kind, UnsupportedShape::Identifier);
        assert_eq!(error.path, format!("#/resources/fixture/args/{name}"));
    }
}

#[test]
fn opaque_values_are_frozen_plain_refs_with_structural_equal_replay() {
    let source = render(&fixture(), &models(), &resources()).expect("fixture");
    assert!(source.contains("freezePlain(structuredClone(dto.opaque))"));
    assert!(source.contains("compareStructural(this.opaque, dto.opaque)"));
}

#[test]
fn nested_tracking_metadata_keeps_only_selected_nested_models() {
    let mut metadata = models();
    metadata[0] = metadata[0].clone().track_nested(&["choice"]);
    let source = render(&fixture(), &metadata, &resources()).expect("fixture");
    assert!(!source.contains("class Row_nestedModel"));
    assert!(source.contains("class Row_choiceVariant0Model"));
    assert!(source.contains("freezePlain(structuredClone(dto.nested))"));
    metadata[0] = metadata[0].clone().track_nested(&["missing"]);
    let error = render(&fixture(), &metadata, &resources()).expect_err("unknown nested field");
    assert_eq!(error.path, "#/components/schemas/Row/properties/missing");
}

#[test]
fn schema_property_names_can_be_typescript_keywords() {
    let mut document = fixture();
    document["components"]["schemas"]["Row"]["properties"]["type"] = json!({"type":"string"});
    render(&document, &models(), &resources()).expect("type is a valid class property");
}

#[test]
fn referenced_scalar_union_fields_and_aliased_resource_roots_render() {
    let mut document = fixture();
    document["components"]["schemas"]["Row"]["properties"]["optional"] =
        json!({"anyOf":[{"$ref":"#/components/schemas/Text"},{"type":"boolean"}]});
    document["components"]["schemas"]["Text"] = json!({"type":"string"});
    document["components"]["schemas"]["AliasList"] = json!({"$ref":"#/components/schemas/List"});
    document["components"]["schemas"]["SubscriptionDataMap"]["properties"]["fixture"] =
        json!({"$ref":"#/components/schemas/AliasList"});
    let mut resources = resources();
    resources[0].data = "AliasList".into();
    let generated = render(&document, &models(), &resources).expect("aliased resource root");
    assert!(generated.contains("new ListModel(dto)"));
}

#[test]
fn mapped_array_resources_derive_optional_numeric_and_boolean_arguments() {
    let mut document = fixture();
    let resource = &mut document["components"]["schemas"]["ResourceRef"]["oneOf"][0];
    resource["properties"]["tail"] = json!({"type":"integer","minimum":1,"maximum":512});
    resource["properties"]["legacy"] = json!({"type":"boolean"});
    document["components"]["schemas"]["SubscriptionDataMap"]["properties"]["fixture"] =
        json!({"type":"array","items":{"$ref":"#/components/schemas/Row"}});
    let mut args = resources()[0].args.clone();
    args.push(ObservableArg {
        name: "tail".into(),
        brand: "TailWindow".into(),
    });
    args.push(ObservableArg {
        name: "legacy".into(),
        brand: "LegacyRoom".into(),
    });
    let generated = render(
        &document,
        &models(),
        &[ObservableResource::mapped("fixture", "fixture", args)],
    )
    .expect("mapped array");
    assert!(generated.contains("tail?: TailWindow"));
    assert!(generated.contains("legacy?: LegacyRoom"));
    assert!(generated.contains("type TailWindow = number &"));
    assert!(generated.contains("type LegacyRoom = boolean &"));
    assert!(generated.contains("reconcileRow(data, dto)"));
}

#[test]
fn declared_wire_fields_are_separate_from_runtime_facades() {
    let mut metadata = models();
    metadata[0].declared_fields = true;
    let fields = render_fields(&fixture(), &metadata, &resources()).expect("pure wire fields");
    assert!(fields.contains("export type RowFields"));
    assert!(!fields.contains("store/"));
    assert!(!fields.contains("ResourceSlot"));
    let facade = render(&fixture(), &metadata, &resources()).expect("resource facades");
    assert!(!facade.contains("export type RowFields"));
}

#[test]
fn keyed_unions_validate_required_string_identity_paths() {
    let mut document = fixture();
    document["components"]["schemas"]["Entry"] = json!({"oneOf":[
        {"type":"object","additionalProperties":false,"required":["source","value"],"properties":{"source":{"type":"string","const":"process"},"value":{"$ref":"#/components/schemas/IdentityValue"}}},
        {"type":"object","additionalProperties":false,"required":["source","value"],"properties":{"source":{"type":"string","const":"task"},"value":{"$ref":"#/components/schemas/IdentityValue"}}}
    ]});
    document["components"]["schemas"]["IdentityValue"] = json!({"type":"object","additionalProperties":false,"required":["id"],"properties":{"id":{"type":"string"}}});
    document["components"]["schemas"]["List"]["properties"]["rows"]["items"] =
        json!({"$ref":"#/components/schemas/Entry"});
    let metadata = vec![ObservableModel::named("Entry").keyed_by(&["source", "value.id"])];
    let source = render(&document, &metadata, &resources()).expect("required string paths");
    assert!(source.contains("reconcileEntry"));
    assert!(source.contains("JSON.stringify([item.source, item.value.id])"));
    for invalid in [json!({"type":"integer"}), json!({"type":["string","null"]})] {
        document["components"]["schemas"]["IdentityValue"]["properties"]["id"] = invalid;
        assert_eq!(
            render(&document, &metadata, &resources()).unwrap_err().kind,
            UnsupportedShape::Identity
        );
    }
    document["components"]["schemas"]["IdentityValue"]["properties"]["id"] =
        json!({"type":"string"});
    document["components"]["schemas"]["IdentityValue"]["required"] = json!([]);
    assert_eq!(
        render(&document, &metadata, &resources()).unwrap_err().kind,
        UnsupportedShape::Identity
    );
}

#[test]
fn union_opaque_fields_follow_declared_variant_properties() {
    let mut document = fixture();
    document["components"]["schemas"]["List"]["properties"]["rows"]["items"] = json!({"type":"object","additionalProperties":false,"required":["choice"],"properties":{"choice":{"$ref":"#/components/schemas/Choice"}}});
    document["components"]["schemas"]["Choice"] = json!({"oneOf":[
        {"type":"object","additionalProperties":false,"required":["kind","input"],"properties":{"kind":{"type":"string","const":"tool"},"input":true}},
        {"type":"object","additionalProperties":false,"required":["kind","text"],"properties":{"kind":{"type":"string","const":"text"},"text":{"type":"string"}}}
    ]});
    let source = render(
        &document,
        &[ObservableModel::named("Choice").opaque("input")],
        &resources(),
    )
    .expect("explicit frozen provider boundary");
    assert!(source.contains("freezePlain(structuredClone(dto.input))"));
    let error = render(
        &document,
        &[ObservableModel::named("Choice").opaque("unknown")],
        &resources(),
    )
    .unwrap_err();
    assert_eq!(error.kind, UnsupportedShape::UnknownField);
}

#[test]
fn string_enum_without_explicit_type_is_a_scalar() {
    let mut document = fixture();
    document["components"]["schemas"]["Row"]["properties"]["title"] =
        json!({"enum":["passed", "failed"]});
    render(&document, &models(), &resources())
        .expect("string-only enum fixes its scalar representation");
}

#[test]
fn all_of_objects_merge_properties_and_required_and_reject_conflicts() {
    let mut document = fixture();
    document["components"]["schemas"]["Row"]["properties"]["nested"] = json!({"allOf":[
        {"type":"object","additionalProperties":false,"properties":{"count":{"type":"number"}},"required":["count"]},
        {"type":"object","properties":{"title":{"type":"string"}},"required":["title"]}
    ]});
    let generated = render(&document, &models(), &resources()).expect("merged object members");
    assert!(generated.contains("this.title = dto.title"));
    document["components"]["schemas"]["Row"]["properties"]["nested"]["allOf"][1]["properties"]["count"] =
        json!({"type":"string"});
    let error =
        render(&document, &models(), &resources()).expect_err("incompatible property declarations");
    assert_eq!(error.kind, UnsupportedShape::Composition);
    assert!(error.path.ends_with("/allOf/1/properties/count"));
}

#[test]
fn collection_inventory_is_derived_from_payload_tags_and_rejects_duplicates() {
    let payload = json!({"type":"object","properties":{"collection":{"const":"row"},"id":{"type":"string"},"value":{"$ref":"#/components/schemas/Row"}}});
    let mut document = fixture();
    document["components"]["schemas"]["SyncEntityPayload"] = json!({"oneOf":[payload]});
    let generated = render_entity_collections(&document).expect("payload inventory");
    assert!(generated.contains("\"row\": new Map()"));
    document["components"]["schemas"]["SyncEntityPayload"]["oneOf"] =
        json!([payload.clone(), payload]);
    assert_eq!(
        render_entity_collections(&document).unwrap_err().kind,
        UnsupportedShape::Union
    );
}

#[test]
fn all_of_reference_siblings_are_not_silently_discarded() {
    let mut document = fixture();
    document["components"]["schemas"]["Base"] = json!({"type":"object","additionalProperties":false,"properties":{"count":{"type":"number"}},"required":["count"]});
    document["components"]["schemas"]["Row"]["properties"]["nested"] = json!({"allOf":[{"$ref":"#/components/schemas/Base","properties":{"count":{"type":"string"}}}]});
    let error = render(&document, &models(), &resources())
        .expect_err("reference sibling constraints must be merged or rejected");
    assert_eq!(error.kind, UnsupportedShape::Composition);
    assert!(error.path.ends_with("/allOf/0/properties"));
}

#[test]
fn mapped_arrays_use_declared_element_fields_without_changing_closed_lists() {
    let mut document = fixture();
    document["components"]["schemas"]["SubscriptionDataMap"]["properties"]["fixture"] =
        json!({"type":"array","items":{"$ref":"#/components/schemas/Row"}});
    let list = vec![ObservableResource::mapped(
        "fixture",
        "fixture",
        resources()[0].args.clone(),
    )];
    let closed = render(&document, &models(), &list).expect("closed list");
    assert!(closed.contains("export type FixtureData = DeepReadonly<ObservableInput<ResourceData<Extract<ResourceRef, {kind: \"fixture\"}>>>>;"));
    document["components"]["schemas"]["Row"]["additionalProperties"] = json!(true);
    let mut metadata = models();
    metadata[0].declared_fields = true;
    let generated = render(&document, &metadata, &list).expect("declared-field list");
    assert!(generated.contains("export type FixtureData = ReadonlyArray<DeepReadonly<import('./observableFieldTypes.ts').RowFields>>;"));
}

fn conversation_fixture() -> Value {
    json!({"components":{"schemas":{
        "Input":{"type":"object","additionalProperties":false,"required":["text"],"properties":{
            "id":{"anyOf":[{"type":"string"},{"type":"null"}]},"text":{"type":"string"},"notice":{"type":"string"}
        }},
        "Page":{"type":"object","additionalProperties":false,"required":["items"],"properties":{
            "items":{"type":"array","items":{"$ref":"#/components/schemas/Input"}}
        }},
        "Legacy":{"type":"array","items":{"$ref":"#/components/schemas/Input"}},
        "ResourceRef":{"oneOf":[{"type":"object","additionalProperties":false,"required":["kind"],"properties":{"kind":{"const":"room"}}}]},
        "SubscriptionDataMap":{"type":"object","required":["room"],"properties":{"room":{"anyOf":[
            {"$ref":"#/components/schemas/Page"},{"$ref":"#/components/schemas/Legacy"}
        ]}}}
    }}})
}
fn conversation_models() -> Vec<ObservableModel> {
    vec![
        ObservableModel::named("Input")
            .declared_fields()
            .identity_or_index("id")
            .omit("notice"),
    ]
}

#[test]
fn array_object_union_mirror_observes_replaced_roots() {
    let document = conversation_fixture();
    let source = render(
        &document,
        &conversation_models(),
        &[ObservableResource::mapped("room", "room", vec![])],
    )
    .expect("existing object/legacy array union");
    assert!(source.contains("Array.isArray(dto)"));
    assert!(
        source.contains("observable.box<"),
        "replaceable mirror roots must notify observers"
    );
    assert!(
        source.contains("deep: false"),
        "nested models retain identity"
    );
    let mut bad = document;
    bad["components"]["schemas"]["Legacy"] = json!({"type":"object","additionalProperties":false,"properties":{"other":{"type":"string"}}});
    assert_eq!(
        render(
            &bad,
            &conversation_models(),
            &[ObservableResource::mapped("room", "room", vec![])]
        )
        .expect_err("ambiguous object/object union")
        .kind,
        UnsupportedShape::Union
    );
}

#[test]
fn optional_input_identity_retains_legacy_positions_and_validates_before_mutation() {
    let source = render(
        &conversation_fixture(),
        &conversation_models(),
        &[ObservableResource::mirror("Page", "page")],
    )
    .expect("optional input ids");
    assert!(source.contains("item.id == null"));
    assert!(source.contains("JSON.stringify(['index', index])"));
    assert!(source.contains("existing.apply(item)"));
    assert!(
        source.find("if (ids.has(id))").expect("duplicate guard")
            < source.find("const next = dto.map").expect("mutation")
    );
    let mut bad = conversation_fixture();
    bad["components"]["schemas"]["Input"]["properties"]["id"] = json!({"type":"number"});
    assert_eq!(
        render(
            &bad,
            &conversation_models(),
            &[ObservableResource::mirror("Page", "page")]
        )
        .expect_err("numeric optional id")
        .kind,
        UnsupportedShape::Identity
    );
}

#[test]
fn omitted_presentation_fields_are_absent_from_models_and_public_field_contracts() {
    let document = conversation_fixture();
    let resources = [ObservableResource::mirror("Page", "page")];
    let source = render(&document, &conversation_models(), &resources).expect("projection");
    let fields =
        render_fields(&document, &conversation_models(), &resources).expect("field contract");
    assert!(!source.contains("this.notice"));
    assert!(!fields.contains("notice:"));
    assert!(fields.contains("text:"));
    let bad = [ObservableModel::named("Input")
        .declared_fields()
        .omit("missing")];
    assert_eq!(
        render(&document, &bad, &resources)
            .expect_err("unknown omitted field")
            .kind,
        UnsupportedShape::UnknownField
    );
}

#[test]
fn mirror_only_reads_require_no_subscription_contract_or_acquiring_facade() {
    let mut document = conversation_fixture();
    document["components"]["schemas"]
        .as_object_mut()
        .expect("schemas")
        .remove("ResourceRef");
    document["components"]["schemas"]
        .as_object_mut()
        .expect("schemas")
        .remove("SubscriptionDataMap");
    let source = render(
        &document,
        &conversation_models(),
        &[ObservableResource::mirror("Page", "oneShot")],
    )
    .expect("one-shot existing schema");
    assert!(source.contains("createOneShotMirror"));
    assert!(!source.contains("oneShotSlot"));
    assert!(!source.contains("oneShot(): ResourceSlot"));
}

#[test]
fn local_models_emit_schema_fields_without_wire_components() {
    let document = json!({"components":{"schemas":{"LocalProjection":{
        "type":"object","additionalProperties":false,"required":["ready","versions"],
        "properties":{"ready":{"type":"boolean"},"versions":{"type":"object","additionalProperties":{"type":"integer"}}}
    }}}});
    let models = vec![
        ObservableModel::named("LocalProjection")
            .local_fields()
            .track_nested(&[]),
    ];
    let resources = vec![ObservableResource::mirror(
        "LocalProjection",
        "localProjection",
    )];
    let fields = render_fields(&document, &models, &resources).expect("local fields render");
    assert!(fields.contains("ready: boolean"));
    assert!(fields.contains("versions: Record<string, number>"));
    assert!(!fields.contains("components['schemas']['LocalProjection']"));
    let generated = render(&document, &models, &resources).expect("local mirror renders");
    assert!(generated.contains("import('./observableFieldTypes.ts').LocalProjectionFields"));
}
