#![allow(
    clippy::disallowed_methods,
    reason = "build script runs in a dedicated compiler process, outside Tokio"
)]
use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

use openapi_to_rust::analysis::{ObjectAdditionalProperties, PropertyConstraints, SchemaType};
use openapi_to_rust::{CodeGenerator, GeneratorConfig, SchemaAnalyzer};

#[path = "src/agent_projection_fields.rs"]
mod agent_projection_fields;

// Derive the narrower client views from the generated full contract. Compare the
// generated Rust types, so even changes in the generator cannot silently drift.
fn agent_view_conversions(source: &str) -> Result<String, Box<dyn Error>> {
    let parsed = syn::parse_file(source)?;
    let fields = |name: &str| -> Result<
        &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
        Box<dyn Error>,
    > {
        parsed
            .items
            .iter()
            .find_map(|item| {
                let syn::Item::Struct(item) = item else {
                    return None;
                };
                if item.ident != name {
                    return None;
                }
                let syn::Fields::Named(fields) = &item.fields else {
                    return None;
                };
                Some(&fields.named)
            })
            .ok_or_else(|| format!("generated {name} is missing or has unnamed fields").into())
    };
    let full = fields("AgentEntityDto")?;
    let mut output = String::new();
    for target in ["AgentSummaryEntityDto", "AgentDetailEntityDto"] {
        let operational = target == "AgentSummaryEntityDto";
        if operational {
            output.push_str("\nimpl AgentSummaryEntityDto {\npub fn from_agent(source: &AgentEntityDto, operational_status: crate::chat_operational_status::AgentOperationalStatus, mark_read: Option<crate::agent_entity_nested_extra::ReadStateDto>, mark_unread: crate::chat_operational_status::MarkUnreadEligibility) -> Self {\nSelf {\n");
        } else {
            output.push_str(&format!("\nimpl From<&AgentEntityDto> for {target} {{\nfn from(source: &AgentEntityDto) -> Self {{\nSelf {{\n"));
        }
        for field in fields(target)? {
            let name = field
                .ident
                .as_ref()
                .ok_or("generated agent field has no name")?;
            if operational {
                match name.to_string().as_str() {
                    "operational_status" | "mark_read" | "mark_unread" => {
                        output.push_str(&format!("{name},\n"));
                        continue;
                    }
                    "in_flight" => {
                        output
                            .push_str("in_flight: source.in_flight.flatten().unwrap_or(false),\n");
                        continue;
                    }
                    "project_server_id" => {
                        output.push_str(
                            "project_server_id: source.project_server_id.clone().flatten(),\n",
                        );
                        continue;
                    }
                    _ => {}
                }
            }
            let original = full
                .iter()
                .find(|candidate| candidate.ident.as_ref() == Some(name))
                .ok_or_else(|| {
                    format!("{target}.{name} has no corresponding AgentEntityDto field")
                })?;
            if original.ty != field.ty {
                return Err(format!(
                    "{target}.{name} type {:?} differs from AgentEntityDto type {:?}",
                    field.ty, original.ty
                )
                .into());
            }
            output.push_str(&format!("{name}: source.{name}.clone(),\n"));
        }
        output.push_str("}\n}\n}\n");
    }
    Ok(output)
}

// Only ingress schemas whose consumers and full constraints have been audited.
const CONSTRAINED_INGRESS: &[&str] = &[
    "CheckpointPreviewRequest",
    "CheckpointRestoreRequest",
    "CheckpointCaptureRequest",
    "ClaudeAddStartRequest",
    "ClaudeCancelRequest",
    "ClaudeCodeRequest",
    "ClaudeStartRequest",
    "IdRequest",
    "McpReconnectRequest",
    "McpSessionBinding",
    "NativeActionRequest",
    "ActionContext",
    "SafetyActionRequest",
    "ProjectDeleteRequest",
    "VoiceEndRequest",
    "VoiceStartRequest",
    "VoiceStatusRequest",
    "VoiceRecordsRequest",
    "WebPushRemoveRequest",
    "WebPushSubscribeRequest",
];

const PATTERN_TYPES: &[(&str, &str)] = &[
    (
        "^[a-z0-9][a-z0-9_-]{0,31}$",
        "studio_runtime_schema::project_ingress::AccountGroup",
    ),
    ("\\S", "crate::ingress_pattern::NonWhitespace"),
    (
        "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$",
        "crate::ingress_pattern::Uuid",
    ),
    (
        "^[^\\s\\p{C}](?:[^\\s\\p{C}]| +[^\\s\\p{C}])*$",
        "studio_runtime_schema::multi_server_ingress::ServerLabel",
    ),
];

pub fn generate() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let spec_path = manifest_dir.join("../runtime-http/openapi.json");
    let output_dir = PathBuf::from(env::var("OUT_DIR")?).join("generated");
    println!("cargo:rerun-if-changed=../runtime-http/openapi.json");
    println!("cargo:rerun-if-changed=openapi_build.rs");

    let spec = fs::read(&spec_path)?;
    // The crate includes this copy (`OPENAPI_TEXT` in lib.rs): its dep-info path is portable.
    fs::write(
        PathBuf::from(env::var("OUT_DIR")?).join("openapi.json"),
        &spec,
    )?;
    let document: studio_json::Value = studio_json::from_slice(&spec)?;
    let mut analyzer = SchemaAnalyzer::new(document.clone())?;
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
        "TaskDetailResource",
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
    analysis
        .schemas
        .get_mut("ChatWorkspaceMode")
        .ok_or("missing chat workspace mode")?
        .schema_type = SchemaType::Primitive {
        rust_type: "studio_runtime_schema::workspace_modes::ChatWorkspaceMode".to_owned(),
        serde_with: None,
    };
    // Draft ingress has a single bounded Rust owner in routes/sync_documents.
    analysis
        .schemas
        .get_mut("NativeLayrCapability")
        .ok_or("missing native Layr capability")?
        .schema_type = SchemaType::Primitive {
        rust_type: "studio_runtime_schema::native_layr::NativeLayrCapability".to_owned(),
        serde_with: None,
    };
    for name in ["DraftPushRequest", "DraftPushRow", "DraftDocumentInput"] {
        analysis
            .schemas
            .remove(name)
            .ok_or("missing draft ingress schema")?;
    }
    for (name, owner) in [
        (
            "AgentOperationalStatus",
            "studio_runtime_schema::chat_operational_status::AgentOperationalStatus",
        ),
        (
            "MarkUnreadEligibility",
            "studio_runtime_schema::chat_operational_status::MarkUnreadEligibility",
        ),
        (
            "AccessSnapshot",
            "studio_runtime_schema::multi_server_access::AccessSnapshot",
        ),
        (
            "AccessClient",
            "studio_runtime_schema::multi_server_access::PublicAccessClient",
        ),
        (
            "DiscoveredServer",
            "studio_runtime_schema::multi_server_access::PublicAccessClient",
        ),
        (
            "InviteSummary",
            "studio_runtime_schema::multi_server_access::InviteSummary",
        ),
        (
            "AccessInvitation",
            "studio_runtime_schema::multi_server_access::AccessInvitation",
        ),
        (
            "AccessInvitationRequest",
            "studio_runtime_schema::multi_server_ingress::AccessInvitation",
        ),
        (
            "InvitationResponse",
            "studio_runtime_schema::multi_server_access::InvitationResponse",
        ),
        (
            "AccessSettings",
            "studio_runtime_schema::multi_server_access::AccessSettings",
        ),
        (
            "ServerIdentity",
            "studio_runtime_schema::multi_server_access::AccessIdentity",
        ),
        (
            "IdentityAlert",
            "studio_runtime_schema::multi_server_access::IdentityAlert",
        ),
        (
            "IdentityAlertCode",
            "studio_runtime_schema::multi_server_access::IdentityAlertCode",
        ),
        (
            "AccessClientKind",
            "studio_runtime_schema::multi_server_access::AccessClientKind",
        ),
        (
            "AccessClientStatus",
            "studio_runtime_schema::multi_server_access::AccessClientStatus",
        ),
        (
            "Reachability",
            "studio_runtime_schema::multi_server_access::Reachability",
        ),
        (
            "InviteStatus",
            "studio_runtime_schema::multi_server_protocol::InviteStatus",
        ),
    ] {
        analysis
            .schemas
            .get_mut(name)
            .ok_or("missing access response schema")?
            .schema_type = SchemaType::Primitive {
            rust_type: owner.into(),
            serde_with: None,
        };
    }
    // The OpenAPI leaves own integer bounds. Preserve JSON number spellings
    // such as 7.0; the pull-output ingress validates them before typed decoding.
    for (name, owner) in [
        (
            "ActiveToolDto",
            "studio_runtime_schema::agent_entity_nested::ActiveToolDto",
        ),
        (
            "AgentActivity",
            "studio_runtime_schema::agent_entity_nested::AgentActivity",
        ),
        (
            "WorkspaceCleanupSummaryDto",
            "studio_runtime_schema::agent_entity_nested::WorkspaceCleanupSummaryDto",
        ),
        (
            "NativeSafetyBufferingRecord",
            "studio_runtime_schema::agent_entity_nested_extra::NativeSafetyBufferingRecord",
        ),
        (
            "AgentStartAttempt",
            "studio_runtime_schema::agent_entity_nested_extra::AgentStartAttempt",
        ),
        (
            "ConvertedFromLeadDto",
            "studio_runtime_schema::agent_entity_nested_extra::ConvertedFromLeadDto",
        ),
        (
            "AgentNativeRelease",
            "studio_runtime_schema::agent_entity_nested_extra::AgentNativeRelease",
        ),
        (
            "NativeReleasePhase",
            "studio_runtime_schema::persistence::NativeReleasePhase",
        ),
        (
            "ContextUsageDto",
            "studio_runtime_schema::agent_entity_nested_extra::ContextUsageDto",
        ),
        (
            "ReadStateDto",
            "studio_runtime_schema::agent_entity_nested_extra::ReadStateDto",
        ),
        (
            "AgentRole",
            "studio_runtime_schema::agent_entity_nested_extra::AgentRole",
        ),
        (
            "AccountTransferSummaryDto",
            "studio_runtime_schema::agent_entity_transfer::AccountTransferSummaryDto",
        ),
        (
            "AccountTransferBlockedDto",
            "studio_runtime_schema::agent_entity_transfer::AccountTransferNoticeDto",
        ),
        (
            "AccountTransferInterruptedDto",
            "studio_runtime_schema::agent_entity_transfer::AccountTransferNoticeDto",
        ),
        (
            "AccountTransferLeftOnSourceDto",
            "studio_runtime_schema::agent_entity_transfer::AccountTransferLeftOnSourceDto",
        ),
        (
            "AccountTransferScope",
            "studio_runtime_schema::agent_entity_transfer::TransferScope",
        ),
        (
            "AccountTransferStatus",
            "studio_runtime_schema::agent_entity_transfer::TransferStatus",
        ),
        (
            "AgentEntityCapacityRetryDto",
            "studio_runtime_schema::agent_entity_nested_extra::AgentEntityCapacityRetryDto",
        ),
        (
            "AgentEntityUsageResumeDto",
            "studio_runtime_schema::agent_entity_nested_extra::AgentEntityUsageResumeDto",
        ),
        (
            "CapacityRetryCause",
            "studio_runtime_schema::continuation_snapshot::CapacityContinuationCause",
        ),
        (
            "AgentNativeStatus",
            "studio_runtime_schema::agent_entity_nested::AgentNativeStatus",
        ),
        (
            "ExecutionSettingsDto",
            "studio_runtime_schema::agent_entity_nested::ExecutionSettingsDto",
        ),
        (
            "ConnectionCheckDto",
            "studio_runtime_schema::agent_entity_native::ConnectionCheckDto",
        ),
        (
            "NativeThreadBlockDto",
            "studio_runtime_schema::agent_entity_native::NativeThreadBlockDto",
        ),
        (
            "NativeTurnErrorDto",
            "studio_runtime_schema::agent_entity_native::NativeTurnErrorDto",
        ),
        (
            "NativeSafetyRetryDto",
            "studio_runtime_schema::agent_entity_native::NativeSafetyRetryDto",
        ),
        (
            "NativeSafetyStage",
            "studio_runtime_schema::agent_entity_native::NativeSafetyStage",
        ),
        (
            "WorkerDefaultsDto",
            "studio_runtime_schema::agent_entity_nested::WorkerDefaultsDto",
        ),
        (
            "RemoteParentLinkDto",
            "studio_runtime_schema::agent_entity_remote::RemoteParentLinkDto",
        ),
        (
            "RemoteWorkerLinkDto",
            "studio_runtime_schema::agent_entity_remote::RemoteWorkerLinkDto",
        ),
        (
            "ReviewDefaultsDto",
            "studio_runtime_schema::agent_entity_remote::ReviewDefaultsDto",
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
    // These union branches are owned by agent_progress, not generated DTOs.
    analysis.schemas.retain(|name, _| {
        name != "ConvertedFromLeadDtoBy"
            && ![
                "AgentEntityStartWaitDtoVariant",
                "AgentNativeStatusError",
                "AgentNativeStatusPhase",
                "AccountChangeVariant",
                "AccountChangeKind",
                "AccountTransferProgressVariant",
                "AccountTransferProgressKind",
            ]
            .iter()
            .any(|prefix| name.starts_with(prefix))
    });
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
    // The answer result and the forward of a sign-in have one owner too.
    for (name, rust_type) in [
        (
            "AnswerResult",
            "studio_runtime_schema::command::AnswerResult",
        ),
        (
            "BrowserForward",
            "studio_runtime_schema::command::BrowserForward",
        ),
    ] {
        analysis
            .schemas
            .get_mut(name)
            .ok_or("missing answer result schema")?
            .schema_type = SchemaType::Primitive {
            rust_type: rust_type.to_owned(),
            serde_with: None,
        };
    }
    analysis.schemas.retain(|name, _| {
        !(name.starts_with("CommandRequest") && name != "CommandRequest")
            && !matches!(
                name.as_str(),
                "CommandSendBody"
                    | "CommandDeleteBody"
                    | "CommandWithdrawBody"
                    | "CommandStopBody"
                    | "CommandLeadMessageBody"
                    | "CommandResponse"
                    | "CommandResponseApplied"
                    | "CommandResponseAnswer"
                    | "CommandResponseLeadMessage"
                    | "CommandResponseDelete"
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
    // Stored statuses admit future strings that the current web schema does not
    // yet name. Keep the schema unchanged until both sides share that owner.
    for name in [
        "AgentEntityDto",
        "AgentSummaryEntityDto",
        "AgentDetailEntityDto",
    ] {
        let SchemaType::Object { properties, .. } = &mut analysis
            .schemas
            .get_mut(name)
            .ok_or("missing agent entity schema")?
            .schema_type
        else {
            return Err("agent entity schema must be an object".into());
        };
        if let Some(property) = properties.get_mut("status") {
            property.schema_type = SchemaType::Primitive {
                rust_type: "String".to_owned(),
                serde_with: None,
            };
        }
        // AgentRecord already owns the normalized gate, which is forwarded
        // without schema filtering or bounds.
        if let Some(property) = properties.get_mut("taskDefaultGate") {
            property.schema_type = SchemaType::Primitive {
                rust_type: "studio_runtime_schema::task_gate::Gate".to_owned(),
                serde_with: None,
            };
        }
        // Stored counters are u64; the legacy projector does not narrow them.
        if let Some(property) = properties.get_mut("tokensUsed") {
            property.schema_type = SchemaType::Primitive {
                rust_type: "u64".to_owned(),
                serde_with: None,
            };
        }
        // Preserve the original integer or floating-point spelling of created.
        if let Some(property) = properties.get_mut("created") {
            property.schema_type = SchemaType::Primitive {
                rust_type: "studio_runtime_schema::NonnegativeNumber".to_owned(),
                serde_with: None,
            };
        }
        // These fields are either unknown stored extensions or legacy unions
        // whose public shape needs the existing schema filter.
        for &field in agent_projection_fields::SCHEMA_FILTERED_AGENT_FIELDS {
            if let Some(property) = properties.get_mut(field) {
                property.schema_type = SchemaType::Primitive {
                    rust_type: "studio_json::Value".to_owned(),
                    serde_with: None,
                };
            }
        }
    }
    // History entries forward the stored JSON without narrowing legacy records.
    // OpenAPI remains the owner of their envelope and the web contract.
    for name in [
        "WorkResultEntityDto",
        "WorkDecisionEntityDto",
        "WorkReleaseEntityDto",
    ] {
        let SchemaType::Object { properties, .. } = &mut analysis
            .schemas
            .get_mut(name)
            .ok_or("missing work history entity schema")?
            .schema_type
        else {
            return Err("work history entity schema must be an object".into());
        };
        for field in ["rootId", "value"] {
            let property = properties
                .get_mut(field)
                .ok_or("missing work history field")?;
            property.schema_type = SchemaType::Primitive {
                rust_type: "studio_json::OwnedRawJson".to_owned(),
                serde_with: None,
            };
            property.nullable = false;
        }
        properties
            .get_mut("index")
            .ok_or("missing work history index")?
            .schema_type = SchemaType::Primitive {
            rust_type: "u64".to_owned(),
            serde_with: None,
        };
    }
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
        rust_type: "studio_json::Value".to_owned(),
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
    for name in CONSTRAINED_INGRESS {
        let SchemaType::Object {
            properties,
            required,
            ..
        } = &mut analysis
            .schemas
            .get_mut(*name)
            .ok_or("missing constrained ingress schema")?
            .schema_type
        else {
            return Err("constrained ingress must be an object".into());
        };
        for (field, property) in properties.iter_mut() {
            if property.nullable
                && let Some(branches) = document["components"]["schemas"][*name]["properties"]
                    [field]["anyOf"]
                    .as_array()
            {
                let non_null: Vec<_> = branches
                    .iter()
                    .filter(|branch| branch["type"] != "null")
                    .collect();
                if branches.len() != 2 || non_null.len() != 1 || !property.constraints.is_empty() {
                    return Err(format!("unsupported nullable constraint in {name}.{field}").into());
                }
                let details: openapi_to_rust::openapi::SchemaDetails =
                    studio_json::from_value(non_null[0].clone())?;
                property.constraints = PropertyConstraints::from_schema_details(&details);
            }
            let constraints = &property.constraints;
            if constraints.exclusive_minimum.is_some()
                || constraints.exclusive_maximum.is_some()
                || constraints.multiple_of.is_some()
                || constraints.min_items.is_some()
                || constraints.max_items.is_some()
                || constraints.unique_items.is_some()
            {
                return Err(format!("unsupported constraint in {name}").into());
            }
            if !constraints.is_empty() {
                let SchemaType::Primitive { rust_type, .. } = &mut property.schema_type else {
                    return Err(format!("unsupported constraint type in {name}").into());
                };
                if let Some(pattern) = &constraints.pattern {
                    if rust_type != "String"
                        || constraints.minimum.is_some()
                        || constraints.maximum.is_some()
                    {
                        return Err(format!("unsupported pattern combination in {name}").into());
                    }
                    let owner = PATTERN_TYPES
                        .iter()
                        .find(|(source, _)| source == pattern)
                        .ok_or_else(|| format!("unknown pattern in {name}: {pattern}"))?
                        .1;
                    *rust_type =
                        if constraints.min_length.is_some() || constraints.max_length.is_some() {
                            let min = constraints.min_length.unwrap_or(0);
                            let max = constraints
                                .max_length
                                .map_or("{ usize::MAX }".to_owned(), |v| v.to_string());
                            format!("crate::ingress_pattern::BoundedPattern<{owner}, {min}, {max}>")
                        } else {
                            owner.to_owned()
                        };
                } else if rust_type == "String"
                    && constraints.minimum.is_none()
                    && constraints.maximum.is_none()
                {
                    let min = constraints.min_length.unwrap_or(0);
                    let max = constraints
                        .max_length
                        .map_or("{ usize::MAX }".to_owned(), |v| v.to_string());
                    *rust_type =
                        format!("studio_runtime_schema::bounded_text::BoundedText<{min}, {max}>");
                } else if rust_type == "i64"
                    && constraints
                        .minimum
                        .is_some_and(|v| v >= 0.0 && v.fract() == 0.0)
                    && constraints.maximum.is_some_and(|v| {
                        v <= studio_client_domain::MAX_SAFE_INTEGER as f64 && v.fract() == 0.0
                    })
                    && constraints.min_length.is_none()
                    && constraints.max_length.is_none()
                {
                    let (Some(min), Some(max)) = (constraints.minimum, constraints.maximum) else {
                        return Err(format!("missing integer bound in {name}").into());
                    };
                    *rust_type = format!(
                        "crate::ingress_pattern::BoundedSafeInteger<{}, {}>",
                        min as u64, max as u64
                    );
                } else {
                    return Err(format!("unsupported constraint type in {name}").into());
                }
            }
            if !required.contains(field) && !property.nullable {
                let SchemaType::Primitive { serde_with, .. } = &mut property.schema_type else {
                    return Err(format!("unsupported optional field in {name}").into());
                };
                if serde_with.is_some() {
                    return Err(format!("unsupported optional codec in {name}").into());
                }
                *serde_with = Some("crate::non_null".into());
            }
        }
    }
    // The generator already emits strict serde for closed untagged branches.
    // Apply that same rule to every explicitly closed object at this boundary.
    for (name, schema) in &analysis.schemas {
        if matches!(&schema.schema_type, SchemaType::Object { additional_properties, variant: None, .. } if additional_properties.denies_unknown_keys())
        {
            analysis.untagged_union_branches.insert(name.clone());
        }
    }
    let config = GeneratorConfig {
        spec_path,
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
        let mut body = body.replace("serde_json::", "studio_json::");
        if module == "types" {
            body.push_str(&agent_view_conversions(&body)?);
        }
        fs::write(path, body)?;
    }
    if modules.iter().any(|module| module == "types") {
        module_lines.push("pub use types::*;".to_owned());
    }
    fs::write(output_dir.join("mod.rs"), module_lines.join("\n") + "\n")?;
    Ok(())
}
