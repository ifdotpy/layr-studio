//! Typed post-commit resource invalidation requests.
use crate::{bounded_text::BoundedText, resource_ref::ResourceRef};
use serde::{Deserialize, Serialize};

/// Maximum notification request identity, in Unicode code points.
pub const REQUEST_ID_MAX_CODEPOINTS: usize = 200;
/// Maximum resource references in one notification.
pub const MAX_RESOURCES: usize = 256;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceNotifyRequest {
    pub request_id: BoundedText<1, REQUEST_ID_MAX_CODEPOINTS>,
    pub resources: ResourceList,
}

impl ResourceNotifyRequest {
    pub fn new(request_id: String, resources: Vec<ResourceRef>) -> Result<Self, &'static str> {
        Ok(Self {
            request_id: request_id.try_into()?,
            resources: ResourceList::new(resources)?,
        })
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct ResourceList(Vec<ResourceRef>);
impl ResourceList {
    pub fn new(resources: Vec<ResourceRef>) -> Result<Self, &'static str> {
        if (1..=MAX_RESOURCES).contains(&resources.len()) {
            Ok(Self(resources))
        } else {
            Err("invalid resource count")
        }
    }
    pub fn as_slice(&self) -> &[ResourceRef] {
        &self.0
    }
}
impl<'de> Deserialize<'de> for ResourceList {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(Vec::<ResourceRef>::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn request_bounds_match_openapi_and_reject_invalid_shapes() {
        let doc: Value =
            serde_json::from_str(include_str!("../../runtime-http/openapi.json")).unwrap();
        let properties = &doc["components"]["schemas"]["ResourceNotifyRequest"]["properties"];
        assert_eq!(
            properties["requestId"]["maxLength"],
            REQUEST_ID_MAX_CODEPOINTS
        );
        assert_eq!(properties["resources"]["maxItems"], MAX_RESOURCES);
        for value in [
            json!({"requestId":"", "resources":[{"kind":"state"}]}),
            json!({"requestId":"r", "resources":[]}),
            json!({"requestId":"r", "resources":[{"kind":"state"}], "extra":true}),
        ] {
            assert!(serde_json::from_value::<ResourceNotifyRequest>(value).is_err());
        }
    }
}
