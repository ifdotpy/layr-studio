//! Render-ready sidebar contracts. Pure projection lives in studio-read-models.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> { Option::<T>::deserialize(d) }

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SidebarChatRow {
    /// Stable chat identity, without units.
    pub id: String,
    /// Display name, in Unicode text.
    pub name: String,
    /// Owning project path, in filesystem path text.
    pub project_path: String,
    /// Existing project identity for binding across server path aliases.
    #[serde(deserialize_with = "nullable")]
    #[schemars(required)]
    pub project_id: Option<String>,
    /// Existing project owner server identity for binding across server path aliases.
    #[serde(deserialize_with = "nullable")]
    #[schemars(required)]
    pub project_server_id: Option<String>,
    /// Project folder identity; null means the project root.
    #[serde(deserialize_with = "nullable")]
    #[schemars(required)]
    pub project_folder: Option<String>,
    /// Project organization revision, in monotonic changes.
    pub project_folder_revision: u32,
    /// Peer team identity; null means no team.
    #[serde(deserialize_with = "nullable")]
    #[schemars(required)]
    pub team_id: Option<String>,
    /// Canonical saved sidebar ordering group key.
    pub group_key: String,
    /// Zero-based saved position; null is unranked and sorts last.
    #[serde(deserialize_with = "nullable")]
    #[schemars(required)]
    pub order_rank: Option<u32>,
    /// Opaque lexicographic authoritative ordering key, computed by studio-read-models.
    pub sort_key: String,
    /// Latest activity timestamp, in seconds since Unix epoch.
    pub activity_at: f64,
    /// Whether the chat is pinned.
    pub pinned: bool,
    /// Whether the chat is archived.
    pub archived: bool,
    /// Provider identity, owned by provider_kind.
    pub provider: crate::ProviderKind,
    /// Provider model identifier; null means not selected.
    #[serde(deserialize_with = "nullable")]
    #[schemars(required)]
    pub model: Option<String>,
    /// Operational indicator, independent of the unread receipt.
    pub indicator: crate::chat_operational_status::ChatOperationalStatus,
    /// Existing completed-result read CAS proof; null means no completed result.
    #[serde(deserialize_with = "nullable")]
    #[schemars(required)]
    pub mark_read: Option<crate::agent_entity_nested_extra::ReadStateDto>,
    /// Eligibility of an explicit mark-unread action.
    pub mark_unread: MarkUnreadEligibility,
    /// Whether a threadless chat without an in-flight turn can move project.
    pub can_change_project: bool,
    /// Whether the chat has no in-flight turn.
    pub can_archive: bool,
    /// Non-time compact visibility reasons; recent activity is checked at render.
    pub compact_pinned: bool,
    /// Lower-case searchable display text, in Unicode text.
    pub search_text: String,
    /// Current epoch workspace copy progress; counts use the progress owner unit.
    #[serde(deserialize_with = "nullable")]
    #[schemars(required)]
    pub copy_progress: Option<crate::layr_progress::CopyProgress>,
    /// Whether the existing stall owner reports waiting for lead action.
    pub waiting_for_lead: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SidebarChatList {
    /// Visible managed lead chats; row order uses the sidebar source order.
    pub rows: Vec<SidebarChatRow>,
}

/// Server-owned reasons for an unavailable mark-unread action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarkUnreadEligibility {
    Allowed,
    Unsupported,
    NoCompletedResult,
    AlreadyUnread,
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_json::json;

    #[test]
    fn sidebar_wire_requires_nullable_fields_and_safe_integer_bounds() {
        let value = json!({
            "id":"chat","name":"Chat","projectPath":"/project","projectId":null,"projectServerId":null,"projectFolder":null,
            "projectFolderRevision":0,"teamId":null,"groupKey":"group","orderRank":null,
            "sortKey":"sort","activityAt":1.5,"pinned":false,"archived":false,
            "provider":"codex","model":null,"indicator":{"kind":"none","label":"Ready"},
            "markRead":null,"markUnread":"no_completed_result","canChangeProject":true,
            "canArchive":true,"compactPinned":true,"searchText":"chat","copyProgress":null,
            "waitingForLead":false
        });
        let row: SidebarChatRow = studio_json::from_value(value.clone()).expect("canonical row");
        assert_eq!(studio_json::to_value(row).expect("row serialization"), value);
        for field in ["projectFolder", "teamId", "orderRank", "model", "copyProgress", "markRead"] {
            let mut missing = value.clone();
            missing.as_object_mut().expect("object").remove(field);
            assert!(studio_json::from_value::<SidebarChatRow>(missing).is_err(), "{field} required");
        }
        let mut invalid = value.clone();
        invalid["extra"] = json!(true);
        assert!(studio_json::from_value::<SidebarChatRow>(invalid).is_err());
        for field in ["projectFolderRevision", "orderRank"] {
            for number in [-1_i64, 4_294_967_296] {
                let mut invalid = value.clone();
                invalid[field] = json!(number);
                assert!(studio_json::from_value::<SidebarChatRow>(invalid).is_err(), "{field}");
            }
        }
    }
}

/// Extend the existing transport unions from this Rust-owned pilot contract.
pub(crate) fn refresh_openapi(openapi: &mut studio_json::Value) -> Result<(), String> {
    use studio_json::json;
    let mut generator = schemars::generate::SchemaSettings::draft2020_12()
        .for_serialize()
        .with(|settings| settings.definitions_path = "/components/schemas".into())
        .into_generator();
    generator.subschema_for::<SidebarChatList>();
    // Existing leaf contracts retain their own schema renderer/normalization.
    for (name, schema) in generator.take_definitions(true) {
        if !openapi["components"]["schemas"].get(&name).is_some()
            || matches!(name.as_str(), "SidebarChatRow" | "SidebarChatList" | "MarkUnreadEligibility")
        {
            openapi["components"]["schemas"][name] = schema;
        }
    }
    let schemas = &mut openapi["components"]["schemas"];
    schemas["SidebarResource"] = json!({"type":"object","additionalProperties":false,"properties":{"kind":{"type":"string","const":"sidebar"}},"required":["kind"],"x-studio-entity-scope":{"collections":["sidebarChatRow"]}});
    schemas["ResourceRef"]["discriminator"]["mapping"]["sidebar"] = json!("#/components/schemas/SidebarResource");
    let resources = schemas["ResourceRef"]["oneOf"].as_array_mut().ok_or("ResourceRef variants missing")?;
    let reference = json!({"$ref":"#/components/schemas/SidebarResource"});
    if !resources.contains(&reference) { resources.push(reference); }
    schemas["SubscriptionDataMap"]["properties"]["sidebar"] = json!({"$ref":"#/components/schemas/SidebarChatList"});
    let required = schemas["SubscriptionDataMap"]["required"].as_array_mut().ok_or("SubscriptionDataMap required missing")?;
    if !required.contains(&json!("sidebar")) { required.push(json!("sidebar")); }
    schemas["SyncSidebarChatRowEntityPayload"] = json!({"type":"object","additionalProperties":false,"properties":{"collection":{"type":"string","const":"sidebarChatRow"},"id":{"type":"string"},"value":{"$ref":"#/components/schemas/SidebarChatRow"}},"required":["collection","id","value"]});
    schemas["SyncEntityPayload"]["discriminator"]["mapping"]["sidebarChatRow"] = json!("#/components/schemas/SyncSidebarChatRowEntityPayload");
    let variants = schemas["SyncEntityPayload"]["oneOf"].as_array_mut().ok_or("payload variants missing")?;
    let reference = json!({"$ref":"#/components/schemas/SyncSidebarChatRowEntityPayload"});
    if !variants.contains(&reference) { variants.push(reference); }
    let variants = schemas["SyncEntity"]["oneOf"].as_array_mut().ok_or("entity variants missing")?;
    if !variants.iter().any(|v| v["properties"]["collection"]["const"] == "sidebarChatRow") {
        variants.push(json!({"type":"object","properties":{"op":{"type":"string","const":"upsert"},"collection":{"type":"string","const":"sidebarChatRow"},"value":{"$ref":"#/components/schemas/SidebarChatRow"}},"required":["op","collection","value"]}));
    }
    for variant in variants {
        if variant["properties"]["op"]["const"] == "delete" {
            let collections = variant["properties"]["collection"]["enum"].as_array_mut().ok_or("delete collections missing")?;
            if !collections.contains(&json!("sidebarChatRow")) { collections.push(json!("sidebarChatRow")); }
        }
    }
    Ok(())
}
