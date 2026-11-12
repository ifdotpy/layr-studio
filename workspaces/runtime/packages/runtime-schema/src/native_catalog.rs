//! Public catalog summaries. Native vendor DTOs remain the provider read owner.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Native app discovery page size, in entries. The response exposes its continuation cursor.
pub const APPS_PER_PAGE: u32 = 100;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeCatalogEntry {
    pub id: String,
    pub name: String,
    pub enabled: Option<bool>,
}

/// Typed identity registered in the legacy resource union by the contract renderer.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum NativeCatalogResource {
    #[serde(rename = "native-catalog")]
    Catalog {
        #[serde(rename = "accountKey")]
        #[schemars(title = "Accountkey")]
        account_key: crate::resource_ref::NonEmpty,
    },
}

/// Catalog fields of the existing capabilities response.
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NativeCatalogCapabilities {
    #[serde(default)]
    pub apps: Vec<NativeCatalogEntry>,
    #[serde(default)]
    pub plugins: Vec<NativeCatalogEntry>,
    #[serde(default)]
    pub apps_cursor: Option<String>,
}

/// This resource carries invalidation only; existing HTTP reads own its data.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct NativeCatalogSubscription {
    #[serde(rename = "native-catalog")]
    pub native_catalog: (),
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_catalog_resource_matches_its_public_contract() {
        let value = studio_json::json!({"kind":"native-catalog","accountKey":"account"});
        let declared: super::NativeCatalogResource =
            studio_json::from_value(value.clone()).expect("declared native catalog resource");
        assert_eq!(studio_json::to_value(declared).unwrap(), value);
        let resource: crate::resource_ref::ResourceRef =
            studio_json::from_value(value.clone()).expect("native catalog resource");
        assert_eq!(
            studio_json::to_value(resource).expect("resource serialization"),
            value
        );
        assert!(
            crate::compiled_validator("ResourceRef")
                .expect("resource validator")
                .is_valid(&value)
        );
    }
}
