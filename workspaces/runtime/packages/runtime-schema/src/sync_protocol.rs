//! Sync protocol identity. This file is the owner; `openapi.json`
//! (`SyncProtocolResponse`) is checked against it and the web copy at the end
//! of `api.ts` is rendered by `studio-contracts`.

/// Version of the workspace identity endpoint; distinct from the entity stream protocol.
pub const SYNC_IDENTITY_PROTOCOL_VERSION: u32 = 2;

/// The single supported sync protocol version.
pub const SYNC_PROTOCOL_VERSION: u32 = 4;
/// `SYNC_PROTOCOL_VERSION` as text, for headers and query values.
pub const SYNC_PROTOCOL_VERSION_TEXT: &str = "4";
/// Scope name of the sync entity stream.
pub const SYNC_ENTITY_SCOPE: &str = "state:entities:v1";

/// One resource's entity selection, owned by OpenAPI resource schema extensions.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionEntityScope {
    pub collections: Vec<String>,
    pub id_field: Option<String>,
    pub agent_field: Option<String>,
    /// Root-only DTOs do not depend on agentSummary team membership.
    #[serde(default = "include_team_members_default")]
    pub include_team_members: bool,
}

fn include_team_members_default() -> bool {
    true
}

/// Parse the same generated scope metadata used by renderer snapshot replacement.
pub fn subscription_entity_scopes()
-> &'static std::collections::BTreeMap<String, SubscriptionEntityScope> {
    static SCOPES: std::sync::OnceLock<
        std::collections::BTreeMap<String, SubscriptionEntityScope>,
    > = std::sync::OnceLock::new();
    SCOPES.get_or_init(|| {
        let document: serde_json::Value =
            serde_json::from_slice(crate::OPENAPI).expect("committed OpenAPI parses");
        entity_scopes_from_document(&document).expect("committed subscription scopes are valid")
    })
}

pub(crate) fn entity_scopes_from_document(
    document: &serde_json::Value,
) -> Result<std::collections::BTreeMap<String, SubscriptionEntityScope>, String> {
    let schemas = &document["components"]["schemas"];
    let mapping = schemas["ResourceRef"]["discriminator"]["mapping"]
        .as_object()
        .ok_or("ResourceRef mapping missing")?;
    let mut scopes = std::collections::BTreeMap::new();
    for (kind, reference) in mapping {
        let name = reference
            .as_str()
            .and_then(|v| v.rsplit('/').next())
            .ok_or("resource schema missing")?;
        if let Some(scope) = schemas[name].get("x-studio-entity-scope") {
            scopes.insert(
                kind.clone(),
                serde_json::from_value(scope.clone()).map_err(|error| error.to_string())?,
            );
        }
    }
    Ok(scopes)
}

/// Serialized global collection set. SQL readers filter before LIMIT.
pub fn global_entity_collections_json() -> &'static str {
    static COLLECTIONS: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    COLLECTIONS.get_or_init(|| {
        serde_json::to_string(&subscription_entity_scopes()["state"].collections)
            .expect("collections serialize")
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn root_only_scope_matches_canonical_dto_predicates() {
        use std::collections::BTreeMap;
        #[derive(serde::Deserialize)]
        struct Document {
            components: Components,
        }
        #[derive(serde::Deserialize)]
        struct Components {
            schemas: BTreeMap<String, Schema>,
        }
        #[derive(serde::Deserialize)]
        struct Schema {
            #[serde(default)]
            properties: BTreeMap<String, Schema>,
            #[serde(default, rename = "allOf")]
            all_of: Vec<Schema>,
            #[serde(rename = "$ref")]
            reference: Option<String>,
            discriminator: Option<Discriminator>,
        }
        #[derive(serde::Deserialize)]
        struct Discriminator {
            mapping: BTreeMap<String, String>,
        }
        fn schema_ref<'a>(schemas: &'a BTreeMap<String, Schema>, reference: &str) -> &'a Schema {
            let name = reference
                .strip_prefix("#/components/schemas/")
                .expect("local schema reference");
            schemas.get(name).expect("referenced schema")
        }
        fn has_property(
            schemas: &BTreeMap<String, Schema>,
            schema: &Schema,
            property: &str,
        ) -> bool {
            schema.properties.contains_key(property)
                || schema.reference.as_ref().is_some_and(|reference| {
                    has_property(schemas, schema_ref(schemas, reference), property)
                })
                || schema
                    .all_of
                    .iter()
                    .any(|part| has_property(schemas, part, property))
        }
        let document: Document =
            serde_json::from_slice(crate::OPENAPI).expect("committed OpenAPI document");
        let schemas = &document.components.schemas;
        let mapping = &schemas
            .get("SyncEntityPayload")
            .expect("payload schema")
            .discriminator
            .as_ref()
            .expect("payload discriminator")
            .mapping;
        for scope in super::subscription_entity_scopes()
            .values()
            .filter(|s| !s.include_team_members)
        {
            for collection in &scope.collections {
                let payload = schema_ref(
                    schemas,
                    mapping
                        .get(collection)
                        .expect("payload discriminator reference"),
                );
                let value = payload.properties.get("value").expect("payload value");
                let dto = schema_ref(
                    schemas,
                    value
                        .reference
                        .as_ref()
                        .expect("canonical value schema reference"),
                );
                assert!(
                    has_property(schemas, dto, "rootId"),
                    "root-only DTO needs rootId: {collection}"
                );
                assert!(
                    !has_property(schemas, dto, "agent"),
                    "root-only DTO must not select team agents: {collection}"
                );
            }
        }
    }

    #[test]
    fn version_text_matches_version() {
        assert_eq!(
            super::SYNC_PROTOCOL_VERSION.to_string(),
            super::SYNC_PROTOCOL_VERSION_TEXT
        );
    }
    #[test]
    fn subscription_resume_cursor_is_typed_and_bounded() {
        use serde_json::json;
        assert!(
            crate::first_error(
                "SubscriptionCursor",
                &json!({"workspaceId":"durable-db","seq":0})
            )
            .expect("cursor schema")
            .is_none()
        );
        for cursor in [
            json!({"workspaceId":"durable-db","seq":-1}),
            json!({"workspaceId":"durable-db","seq":9007199254740992_u64}),
            json!({"workspaceId":"durable-db","seq":"1"}),
            json!({"workspaceId":"","seq":1}),
            json!({"seq":1}),
            json!({"workspaceId":"durable-db","seq":1,"epoch":"volatile"}),
        ] {
            assert!(
                crate::first_error("SubscriptionCursor", &cursor)
                    .expect("cursor schema")
                    .is_some(),
                "{cursor}"
            );
        }
        for kind in ["snapshot", "delta"] {
            assert!(
                crate::first_error("SubscriptionReplyKind", &json!(kind))
                    .expect("reply kind schema")
                    .is_none()
            );
        }
        assert!(
            crate::first_error("SubscriptionReplyKind", &json!("unknown"))
                .expect("reply kind schema")
                .is_some()
        );
    }

    #[test]
    fn entity_v4_schema_requires_typed_values_and_omits_delete_values() {
        use serde_json::json;
        let valid = json!({"op":"upsert","collection":"workspace","id":"current","seq":1,"value":{"stateDir":"temporary"}});
        assert!(
            crate::first_error("SyncEntity", &valid)
                .expect("schema lookup")
                .is_none()
        );
        let delete = json!({"op":"delete","collection":"agent","id":"a","seq":2});
        assert!(
            crate::first_error("SyncEntity", &delete)
                .expect("schema lookup")
                .is_none()
        );
        for invalid in [
            json!({"op":"upsert","collection":"workspace","id":"current","seq":1,"value":"{}"}),
            json!({"op":"upsert","collection":"workspace","id":"current","seq":1}),
            json!({"op":"upsert","collection":"workspace","id":"current","seq":1,"value":{"stateDir":42}}),
            json!({"op":"delete","collection":"agent","id":"a","seq":2,"value":{}}),
            json!({"op":"delete","collection":"unknown","id":"a","seq":2}),
            json!({"op":"delete","collection":"agent","id":"a","seq":2,"payload":"{}"}),
        ] {
            assert!(
                crate::first_error("SyncEntity", &invalid)
                    .expect("schema lookup")
                    .is_some(),
                "{invalid}"
            );
        }
    }
}
