//! Validated review targets and their workspace assignment.
pub use crate::records::WorkspaceMode;
use serde::{Deserialize, Deserializer, Serialize};

macro_rules! identifier {
    ($name:ident, $valid:expr, $schema:expr) => {
        #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);
        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
        impl TryFrom<String> for $name {
            type Error = &'static str;
            fn try_from(value: String) -> Result<Self, Self::Error> {
                if ($valid)(&value) {
                    Ok(Self(value))
                } else {
                    Err(concat!("invalid ", stringify!($name)))
                }
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                Self::try_from(String::deserialize(d)?).map_err(serde::de::Error::custom)
            }
        }
        impl schemars::JsonSchema for $name {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                stringify!($name).into()
            }
            fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
                $schema
            }
        }
    };
}
fn hex_id(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
identifier!(
    LayrStateId,
    |v: &str| hex_id(v, 32),
    schemars::json_schema!({"type":"string","pattern":"^[0-9a-f]{32}$"})
);
identifier!(
    GitCommit,
    |v: &str| hex_id(v, 40),
    schemars::json_schema!({"type":"string","pattern":"^[0-9a-f]{40}$"})
);
identifier!(
    GitRef,
    |v: &str| !v.is_empty(),
    schemars::json_schema!({"type":"string","minLength":1})
);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum LayrBase {
    MergeBaseWithMain,
    State { state: LayrStateId },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ReviewTarget {
    LayrState { state: LayrStateId, base: LayrBase },
    GitRange { base: GitRef, head: GitCommit },
}

/// A target whose workspace mode has been checked at ingress.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(try_from = "ReviewTarget", into = "ReviewTarget")]
pub struct ReviewAssignment(ReviewTarget);

#[derive(Clone, Debug, thiserror::Error)]
#[error("Review target does not match the project workspace")]
pub struct ReviewRefusal;

impl ReviewAssignment {
    pub fn new(mode: WorkspaceMode, target: ReviewTarget) -> Result<Self, ReviewRefusal> {
        match (mode, &target) {
            (WorkspaceMode::LayrNative, ReviewTarget::LayrState { .. })
            | (WorkspaceMode::Worktree, ReviewTarget::GitRange { .. }) => Ok(Self(target)),
            _ => Err(ReviewRefusal),
        }
    }
    pub fn target(&self) -> &ReviewTarget {
        &self.0
    }
}
impl TryFrom<ReviewTarget> for ReviewAssignment {
    type Error = ReviewRefusal;
    fn try_from(target: ReviewTarget) -> Result<Self, Self::Error> {
        let mode = match target {
            ReviewTarget::LayrState { .. } => WorkspaceMode::LayrNative,
            ReviewTarget::GitRange { .. } => WorkspaceMode::Worktree,
        };
        Self::new(mode, target)
    }
}
impl From<ReviewAssignment> for ReviewTarget {
    fn from(value: ReviewAssignment) -> Self {
        value.0
    }
}

/// Embed the same target schema in both managed review entry points.
pub fn apply_tool_contract(
    name: &str,
    schema: &mut studio_json::Map<String, studio_json::Value>,
) -> Result<(), &'static str> {
    let pointer = match name {
        "orchestration_spawn" => "/agents/items/properties",
        "orchestration_review" => "",
        _ => return Ok(()),
    };
    let field = if name == "orchestration_spawn" {
        "review"
    } else {
        "target"
    };
    let properties = schema
        .get_mut("properties")
        .and_then(|value| value.pointer_mut(pointer))
        .and_then(studio_json::Value::as_object_mut)
        .ok_or("tool schema")?;
    let schema = schemars::generate::SchemaSettings::draft2020_12()
        .with(|settings| settings.inline_subschemas = true)
        .into_generator()
        .into_root_schema_for::<ReviewTarget>()
        .to_value();
    properties.insert(field.into(), schema);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn assignment_validates_ids_and_workspace() {
        let layr: ReviewTarget = studio_json::from_str(r#"{"kind":"layrState","state":"0123456789abcdef0123456789abcdef","base":{"kind":"mergeBaseWithMain"}}"#).unwrap();
        let git: ReviewTarget = studio_json::from_str(r#"{"kind":"gitRange","base":"main","head":"0123456789abcdef0123456789abcdef01234567"}"#).unwrap();
        assert!(ReviewAssignment::new(WorkspaceMode::LayrNative, layr.clone()).is_ok());
        assert!(ReviewAssignment::new(WorkspaceMode::Worktree, git.clone()).is_ok());
        for mode in [
            WorkspaceMode::Shared,
            WorkspaceMode::Layr,
            WorkspaceMode::Image,
        ] {
            assert!(ReviewAssignment::new(mode, layr.clone()).is_err());
            assert!(ReviewAssignment::new(mode, git.clone()).is_err());
        }
        assert!(ReviewAssignment::new(WorkspaceMode::Worktree, layr).is_err());
        assert!(ReviewAssignment::new(WorkspaceMode::LayrNative, git).is_err());
        for invalid in [
            "",
            "0123456789abcdef0123456789abcdeF",
            "0123456789abcdef0123456789abcde",
            "01234567-89ab-cdef-0123-456789abcdef",
        ] {
            assert!(LayrStateId::try_from(invalid.to_owned()).is_err());
        }
        assert!(GitCommit::try_from("f".repeat(39)).is_err());
        assert!(GitRef::try_from(String::new()).is_err());
    }
}
