use super::*;
use studio_json::json;

fn fixture() -> Value {
    json!({"components":{"schemas":{
        "Row":{"type":"object","additionalProperties":false,"required":["id","sortKey","title","nested","tags","choice","opaque"],"properties":{
            "id":{"type":"string"},"sortKey":{"type":"string"},"title":{"type":["string","null"]},
            "nested":{"type":"object","additionalProperties":false,"required":["count"],"properties":{"count":{"type":"number"}}},
            "tags":{"type":"array","items":{"type":"string"}},
            "optional":{"type":"string"},
            "opaque":{"type":"object","additionalProperties":true},
            "choice":{"oneOf":[
                {"type":"object","additionalProperties":false,"required":["kind","count"],"properties":{"kind":{"type":"string","enum":["count"]},"count":{"type":"integer"}}},
                {"type":"object","additionalProperties":false,"required":["kind","text"],"properties":{"kind":{"type":"string","enum":["text"]},"text":{"type":"string"}}}
            ]}
        }},
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
    for invalid in [
        json!({}),
        json!({"type":"object","additionalProperties":true}),
        json!({"allOf":[{"type":"string"}]}),
        json!({"type":["string","number"]}),
    ] {
        let mut bad = document.clone();
        bad["components"]["schemas"]["Row"]["properties"]["nested"] = invalid;
        let error = render(&bad, &models(), &resources()).expect_err("unsupported");
        assert!(error.path.contains("/Row/properties/nested"), "{error}");
        assert_eq!(error.kind, UnsupportedShape::Shape);
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
