//! Shared account identity used by transfer ingress and projections.

/// Maximum account identity length, in Unicode code points.
pub const ACCOUNT_KEY_MAX_CHARS: usize = 200;

/// A nonempty account identity, at most 200 Unicode code points.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct AccountKey(crate::bounded_text::BoundedText<1, ACCOUNT_KEY_MAX_CHARS>);

impl schemars::JsonSchema for AccountKey {
    fn inline_schema() -> bool {
        true
    }
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "AccountKey".into()
    }
    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        <crate::bounded_text::BoundedText<1, ACCOUNT_KEY_MAX_CHARS> as schemars::JsonSchema>::json_schema(generator)
    }
}

impl AccountKey {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
impl TryFrom<String> for AccountKey {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(Self(value.try_into()?))
    }
}
