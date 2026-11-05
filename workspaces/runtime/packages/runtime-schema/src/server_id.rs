//! The id of a Studio server, as its identity, peer links and project homes name it.

/// A Studio server id. On the wire it is the plain string.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct ServerId(pub String);

impl schemars::JsonSchema for ServerId {
    fn inline_schema() -> bool {
        true
    }
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ServerId".into()
    }
    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        <String as schemars::JsonSchema>::json_schema(generator)
    }
}

impl ServerId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for ServerId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ServerId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<String> for ServerId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ServerId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl PartialEq<str> for ServerId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for ServerId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for ServerId {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl PartialEq<ServerId> for str {
    fn eq(&self, other: &ServerId) -> bool {
        self == other.0
    }
}

impl PartialEq<ServerId> for &str {
    fn eq(&self, other: &ServerId) -> bool {
        *self == other.0
    }
}

impl PartialEq<ServerId> for String {
    fn eq(&self, other: &ServerId) -> bool {
        *self == other.0
    }
}
