#![allow(
    clippy::disallowed_methods,
    reason = "build script runs in a dedicated compiler process, outside Tokio"
)]
use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

use openapi_to_rust::analysis::{ObjectAdditionalProperties, SchemaType};
use openapi_to_rust::{CodeGenerator, GeneratorConfig, SchemaAnalyzer};
fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    // Clippy does not merge parent configs: runtime/clippy.toml owns the shared policy.
    let shared_clippy = manifest_dir.join("../../clippy.toml");
    let http_clippy = manifest_dir.join("clippy.toml");
    // Relative paths: absolute ones differ in every layr line and force a rebuild.
    println!("cargo:rerun-if-changed=../../clippy.toml");
    println!("cargo:rerun-if-changed=clippy.toml");
    let shared = fs::read_to_string(shared_clippy)?;
    let http = fs::read_to_string(http_clippy)?;
    let inherited = http
        .lines()
        .filter(|line| {
            !line.contains("HTTP-only blocking filesystem guards")
                && !line.contains("HTTP handlers must use ")
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    assert_eq!(
        shared, inherited,
        "HTTP Clippy config must preserve the policy owned by runtime/clippy.toml"
    );
    let spec_path = manifest_dir.join("openapi.json");
    let output_dir = PathBuf::from(env::var("OUT_DIR")?).join("generated");
    println!("cargo:rerun-if-changed=openapi.json");
    println!("cargo:rerun-if-changed=build.rs");

    let document: serde_json::Value = serde_json::from_slice(&fs::read(&spec_path)?)?;
    let mut analyzer = SchemaAnalyzer::new(document)?;
    let mut analysis = analyzer.analyze()?;
    analysis
        .schemas
        .get_mut("AgentCapacity")
        .ok_or("missing agent capacity schema")?
        .schema_type = SchemaType::Primitive {
        rust_type: "studio_runtime_schema::agent_limits::AgentCapacity".to_owned(),
        serde_with: None,
    };
    analysis
        .schemas
        .get_mut("ResourceRef")
        .ok_or("missing resource identity schema")?
        .schema_type = SchemaType::Primitive {
        rust_type: "studio_runtime_schema::resource_ref::ResourceRef".to_owned(),
        serde_with: None,
    };
    // The resource union owns these payloads; progress events still use their
    // separately generated resource schemas until those events are migrated.
    for name in [
        "AccountsResource",
        "CostsResource",
        "DesktopResource",
        "DraftsResource",
        "LimitsResource",
        "ModelsResource",
        "MultiServerResource",
        "QueueResource",
        "ReceiptsResource",
        "RoomResource",
        "SessionCostResource",
        "StateResource",
        "TaskResource",
        "TasksResource",
        "TerminalResource",
        "TerminalsResource",
        "TranscriptResource",
        "TranscriptsResource",
        "VoiceResource",
        "WorkspaceResource",
        "AgentDetailResource",
        "MonitorsResource",
        "ProcessesResource",
        "WorkResource",
        "ComplaintResource",
        "TerminalOutputResource",
    ] {
        analysis
            .schemas
            .remove(name)
            .ok_or("missing resource payload schema")?;
        drop(analysis.schemas.remove(&format!("{name}Kind")));
    }
    analysis
        .schemas
        .get_mut("ResetRequest")
        .ok_or("missing reset request schema")?
        .schema_type = SchemaType::Primitive {
        rust_type: "studio_runtime_schema::limit_reset_request::ResetRequest".to_owned(),
        serde_with: None,
    };
    // Draft ingress has a single bounded Rust owner in routes/sync_documents.
    for name in ["DraftPushRequest", "DraftPushRow", "DraftDocumentInput"] {
        analysis
            .schemas
            .remove(name)
            .ok_or("missing draft ingress schema")?;
    }
    // The OpenAPI leaves own integer bounds. Preserve JSON number spellings
    // such as 7.0; the pull-output ingress validates them before typed decoding.
    for (name, owner) in [
        (
            "WorkerDefaultsDto",
            "studio_runtime_schema::agent_entity_nested::WorkerDefaultsDto",
        ),
        (
            "AgentNativeStatusValue",
            "studio_runtime_schema::agent_entity_nested::AgentNativeStatusValue",
        ),
        (
            "AgentEntityContextRepairWaitDto",
            "studio_runtime_schema::agent_entity_nested::AgentEntityContextRepairWaitDto",
        ),
        (
            "MovePointerDto",
            "studio_runtime_schema::agent_entity_nested::MovePointerDto",
        ),
        (
            "AgentMode",
            "studio_runtime_schema::agent_entity_nested::AgentMode",
        ),
        (
            "AgentStatus",
            "studio_runtime_schema::agent_entity_nested::AgentStatus",
        ),
        (
            "AgentNaming",
            "studio_runtime_schema::agent_entity_nested::AgentNaming",
        ),
        (
            "AccountChange",
            "studio_runtime_schema::agent_progress::AccountChange",
        ),
        (
            "AccountDestination",
            "studio_runtime_schema::account_destination::AccountDestination",
        ),
        (
            "AccountTransferProgress",
            "studio_runtime_schema::agent_progress::AccountTransferProgress",
        ),
        (
            "NativeProviderError",
            "studio_runtime_schema::provider_errors::NativeProviderError",
        ),
        (
            "AgentOverview",
            "studio_runtime_schema::agent_entity_nested::AgentOverview",
        ),
        (
            "AgentSource",
            "studio_runtime_schema::agent_entity_nested::AgentSource",
        ),
        (
            "AgentCommandProgress",
            "studio_runtime_schema::agent_progress::AgentCommandProgress",
        ),
        (
            "AgentStall",
            "studio_runtime_schema::agent_progress::AgentStall",
        ),
        (
            "AgentEntityStartWaitDto",
            "studio_runtime_schema::agent_progress::AgentStartWait",
        ),
        (
            "TeamCounts",
            "studio_runtime_schema::agent_progress::TeamCounts",
        ),
        ("VaultPullSequence", "serde_json::Number"),
        ("VaultPullFiles", "serde_json::Number"),
        ("VaultPullBytes", "serde_json::Number"),
        (
            "ProviderModel",
            "studio_runtime_schema::model_catalog::ProviderModel",
        ),
        (
            "ProcessListing",
            "studio_runtime_schema::process_controls::ProcessListing",
        ),
        (
            "WorkBody",
            "studio_runtime_schema::task_command::TaskCommand",
        ),
        ("WorkStatus", "studio_runtime_schema::records::WorkStatus"),
        (
            "WorkspaceMode",
            "studio_runtime_schema::records::WorkspaceMode",
        ),
    ] {
        analysis
            .schemas
            .get_mut(name)
            .ok_or("missing owned record domain")?
            .schema_type = SchemaType::Primitive {
            rust_type: owner.into(),
            serde_with: None,
        };
    }
    // The start-wait union is owned by agent_progress; its generated branches
    // are no longer referenced by the mapped component.
    analysis
        .schemas
        .retain(|name, _| !name.starts_with("AgentEntityStartWaitDtoVariant"));
    analysis
        .schemas
        .get_mut("MonitorState")
        .ok_or("missing monitor state schema")?
        .schema_type = SchemaType::Primitive {
        rust_type: "studio_runtime_schema::studio_errors::MonitorStatus".into(),
        serde_with: None,
    };
    analysis
        .schemas
        .get_mut("StudioError")
        .ok_or("missing incident schema")?
        .schema_type = SchemaType::Primitive {
        rust_type: "studio_runtime_schema::studio_errors::StudioError".into(),
        serde_with: None,
    };
    // HTTP identities use the schema owner's enum, including through references.
    analysis
        .schemas
        .get_mut("ProviderKind")
        .ok_or("missing provider identity schema")?
        .schema_type = SchemaType::Primitive {
        rust_type: "studio_runtime_schema::ProviderKind".to_owned(),
        serde_with: None,
    };
    // Alias the named component to its runtime owner. References retain their
    // optional/null wrappers without generating a second status enum.
    analysis
        .schemas
        .get_mut("ComplaintResponseStatus")
        .ok_or("missing complaint response status schema")?
        .schema_type = SchemaType::Primitive {
        rust_type: "studio_runtime_schema::complaints::ComplaintResponseStatus".to_owned(),
        serde_with: None,
    };
    // Queue ingress has one shared serde owner. Keep the generated request
    // name as an alias, and do not emit unused parallel payload DTOs.
    analysis
        .schemas
        .get_mut("QueueActionApiQueuePostRequest")
        .ok_or("missing queue command schema")?
        .schema_type = SchemaType::Primitive {
        rust_type: "studio_runtime_schema::queue_command::QueueCommand".to_owned(),
        serde_with: None,
    };
    for name in [
        "QueueEditBody",
        "QueueFirstBody",
        "QueueReorderBody",
        "QueueEditBodyAction",
        "QueueFirstBodyAction",
        "QueueReorderBodyAction",
    ] {
        analysis
            .schemas
            .remove(name)
            .ok_or("missing queue payload schema")?;
    }
    // The command envelope has one serde owner. Keep the generated name as an alias and do
    // not emit parallel DTOs for its variants and bodies. The response is built from the
    // owner's `CommandResponse`, so its generated copy is unused and not emitted.
    analysis
        .schemas
        .get_mut("CommandRequest")
        .ok_or("missing command request schema")?
        .schema_type = SchemaType::Primitive {
        rust_type: "studio_runtime_schema::command::CommandRequest".to_owned(),
        serde_with: None,
    };
    analysis.schemas.retain(|name, _| {
        !(name.starts_with("CommandRequest") && name != "CommandRequest")
            && !matches!(
                name.as_str(),
                "CommandSendBody"
                    | "CommandWithdrawBody"
                    | "CommandStopBody"
                    | "CommandResponse"
                    | "CommandRow"
            )
    });
    // Claude settings deserialize through their validated tagged command owner.
    // The session route owns its strict request union directly.
    analysis
        .schemas
        .remove("ClaudeSessionApiClaudeSessionPostRequest");
    for (name, owner) in [
        (
            "ClaudeSessionSettingsRequest",
            "studio_runtime_schema::claude_controls::ClaudeControlCommand",
        ),
        (
            "ClaudeSettings",
            "studio_runtime_schema::claude_controls::ClaudeSettings",
        ),
    ] {
        analysis
            .schemas
            .get_mut(name)
            .ok_or("missing Claude control schema")?
            .schema_type = SchemaType::Primitive {
            rust_type: owner.to_owned(),
            serde_with: None,
        };
    }
    for name in [
        "ClaudeSessionSettingsRequestAction",
        "ClaudeSettingsPermissionMode",
    ] {
        analysis
            .schemas
            .remove(name)
            .ok_or("missing Claude control leaf schema")?;
    }
    // Keep the wire schema shared, and deserialize ingress timing directly
    // into its runtime owner through the explicit compatibility codec.
    let SchemaType::Object { properties, .. } = &mut analysis
        .schemas
        .get_mut("ManagedMessageBody")
        .ok_or("missing managed message schema")?
        .schema_type
    else {
        return Err("managed message schema must be an object".into());
    };
    properties
        .get_mut("delivery")
        .ok_or("missing managed message delivery")?
        .schema_type = SchemaType::Primitive {
        rust_type: "studio_runtime_schema::DeliveryMoment".to_owned(),
        serde_with: Some("studio_runtime_schema::delivery::compatibility".to_owned()),
    };
    // Normalization already validates each collection DTO. Keep its parsed value
    // in place rather than deserialize it into a second owned object tree. The
    // OpenAPI oneOf still generates the fully typed TypeScript discriminated union.
    let SchemaType::Object {
        properties: payload_properties,
        ..
    } = &analysis
        .schemas
        .get("SyncAgentEntityPayload")
        .ok_or("missing entity payload schema")?
        .schema_type
    else {
        return Err("entity payload schema must be an object".into());
    };
    let mut value_property = payload_properties
        .get("value")
        .ok_or("missing entity value")?
        .clone();
    value_property.schema_type = SchemaType::Primitive {
        rust_type: "serde_json::Value".to_owned(),
        serde_with: None,
    };
    let SchemaType::Object {
        properties,
        variant,
        additional_properties,
        ..
    } = &mut analysis
        .schemas
        .get_mut("SyncEntity")
        .ok_or("missing sync entity schema")?
        .schema_type
    else {
        return Err("sync entity schema must be an object".into());
    };
    *variant = None;
    // unevaluatedProperties:false closes the complete schema after its oneOf.
    *additional_properties = ObjectAdditionalProperties::Denied;
    properties.insert("value".to_owned(), value_property);
    // The generator already emits strict serde for closed untagged branches.
    // Apply that same rule to every explicitly closed object at this boundary.
    for (name, schema) in &analysis.schemas {
        if matches!(&schema.schema_type, SchemaType::Object { additional_properties, variant: None, .. } if additional_properties.denies_unknown_keys())
        {
            analysis.untagged_union_branches.insert(name.clone());
        }
    }
    let config = GeneratorConfig {
        spec_path: spec_path.clone(),
        output_dir: output_dir.clone(),
        module_name: "generated".to_owned(),
        enable_async_client: false,
        enable_sse_client: false,
        tracing_enabled: false,
        ..GeneratorConfig::default()
    };
    let generator = CodeGenerator::new(config)
        .with_source_provenance("workspaces/runtime/packages/runtime-http/openapi.json");
    let generated = generator.generate_all(&mut analysis)?;
    let mut module_lines = Vec::new();
    let mut modules = Vec::new();
    for file in generated.files {
        let path = output_dir.join(file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let module = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("generated module has an invalid filename")?;
        modules.push(module.to_owned());
        // `include!` rejects inner attributes: hoist `#![x]` to `#[x]` on the module and
        // drop the `//!` header. The path stays relative to OUT_DIR, not an absolute path.
        let mut attributes = String::new();
        let mut body = String::new();
        for line in file.content.lines() {
            if let Some(attribute) = line.strip_prefix("#![") {
                attributes.push_str(&format!("#[{attribute} "));
            } else if !line.starts_with("//!") {
                body.push_str(line);
                body.push('\n');
            }
        }
        module_lines.push(format!(
            "{attributes}pub mod {module} {{ include!(concat!(env!(\"OUT_DIR\"), \"/generated/{module}.rs\")); }}"
        ));
        fs::write(path, body)?;
    }
    if modules.iter().any(|module| module == "types") {
        module_lines.push("pub use types::*;".to_owned());
    }
    fs::write(output_dir.join("mod.rs"), module_lines.join("\n") + "\n")?;
    Ok(())
}
