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
        ordered_by: Some("sortKey".into()),
        opaque: vec!["opaque".into()],
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
    assert!(!generated.contains("ignoredModel"));
    assert_witness(&document, &generated, true);
    let mut bad = resources.clone();
    bad[0].collections[0].schema = "List".into();
    assert_eq!(
        render(&document, &models(), &bad)
            .expect_err("mismatched selected value")
            .kind,
        UnsupportedShape::Resource
    );
    bad = resources.clone();
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
    }]
}

#[test]
fn deterministic_fixture_and_typed_path_failures() {
    let document = fixture();
    let generated = render(&document, &models(), &resources()).expect("fixture renders");
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
    let mut bad_field = document.clone();
    bad_field["components"]["schemas"]["Row"]["properties"]["apply"] = json!({"type":"string"});
    let error = render(&bad_field, &models(), &resources()).expect_err("method collision");
    assert_eq!(error.kind, UnsupportedShape::Identifier);
    assert_eq!(error.path, "#/components/schemas/Row/properties/apply");
}

#[test]
fn generated_fixture_compiles_and_updates_through_facade() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let document = fixture();
    let generated = render(&document, &models(), &resources()).expect("fixture");
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
            studio_json::to_string(&json!({"source":generated,"document":document}))
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
