//! Project mutation request bodies.
use crate::bounded_text::BoundedText;
use crate::queue_command::Field;
use crate::recovery_record::SafeInteger;
use serde::{Deserialize, Deserializer};
use std::sync::LazyLock;

#[derive(Clone, Debug)]
pub struct AccountGroup(String);

impl AccountGroup {
    pub fn is_valid(value: &str) -> bool {
        static PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
            // The shared pattern is a checked-in constant.
            regex::Regex::new(crate::shared_values::ACCOUNT_GROUP_PATTERN)
                .expect("valid account group pattern")
        });
        PATTERN.is_match(value)
    }
    pub fn into_string(self) -> String {
        self.0
    }
}
impl<'de> Deserialize<'de> for AccountGroup {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if Self::is_valid(&value) {
            Ok(Self(value))
        } else {
            Err(serde::de::Error::custom("invalid account group"))
        }
    }
}

fn account_groups<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<AccountGroup>>, D::Error> {
    let groups = Option::<Vec<AccountGroup>>::deserialize(deserializer)?;
    if groups
        .as_ref()
        .is_some_and(|groups| groups.len() > crate::shared_values::ACCOUNT_GROUPS_MAX)
    {
        return Err(serde::de::Error::custom("too many account groups"));
    }
    Ok(groups)
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectAction {
    Register,
    Remove,
    SetAccount,
    SetWorkerBase,
    SetWorkerEnvironment,
    SetAccounts,
    Rename,
    AddFolder,
    RenameFolder,
    RemoveFolder,
    SetAccountGroups,
    SetWorkspacePolicy,
}

impl ProjectAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Register => "register",
            Self::Remove => "remove",
            Self::SetAccount => "set_account",
            Self::SetWorkerBase => "set_worker_base",
            Self::SetWorkerEnvironment => "set_worker_environment",
            Self::SetAccounts => "set_accounts",
            Self::Rename => "rename",
            Self::AddFolder => "add_folder",
            Self::RenameFolder => "rename_folder",
            Self::RemoveFolder => "remove_folder",
            Self::SetAccountGroups => "set_account_groups",
            Self::SetWorkspacePolicy => "set_workspace_policy",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkerEnvironment {
    Host,
    Linux,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub enum WorkspacePolicy {
    #[serde(rename = "layr-native")]
    LayrNative,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectWrite {
    pub path: BoundedText<1, { usize::MAX }>,
    pub action: Option<ProjectAction>,
    #[serde(default, deserialize_with = "account_groups")]
    pub account_groups: Option<Vec<AccountGroup>>,
    pub account_key: Option<String>,
    pub account_keys: Option<Vec<String>>,
    pub base_ref: Option<String>,
    pub environment: Option<WorkerEnvironment>,
    pub expected_revision: Option<SafeInteger>,
    pub folder_id: Option<String>,
    #[serde(default)]
    pub name: Field<String>,
    pub parent_id: Option<String>,
    pub request_id: Option<BoundedText<1, 128>>,
    pub server: Option<BoundedText<0, 128>>,
    #[serde(default)]
    pub workspace_policy: Field<WorkspacePolicy>,
}

impl ProjectWrite {
    pub fn register(path: String, name: Option<String>) -> Result<Self, &'static str> {
        Ok(Self {
            path: path.try_into()?,
            action: Some(ProjectAction::Register),
            account_groups: None,
            account_key: None,
            account_keys: None,
            base_ref: None,
            environment: None,
            expected_revision: None,
            folder_id: None,
            name: name.map_or(Field::Missing, Field::Value),
            parent_id: None,
            request_id: None,
            server: None,
            workspace_policy: Field::Missing,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocationAction {
    AddLocation,
    RemoveLocation,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocationWrite {
    pub action: LocationAction,
    pub project: BoundedText<1, 4096>,
    pub server: BoundedText<1, 128>,
    pub request_id: BoundedText<1, 128>,
    pub path: Option<BoundedText<0, 4096>>,
}

#[cfg(test)]
mod tests {
    use super::{AccountGroup, ProjectWrite};
    use serde::Deserialize;

    #[test]
    fn account_group_uses_shared_pattern() {
        assert!(serde_json::from_str::<AccountGroup>(r#""team_1""#).is_ok());
        assert!(serde_json::from_str::<AccountGroup>(r#""Team""#).is_err());
        assert!(serde_json::from_str::<AccountGroup>(r#""""#).is_err());
        assert!(AccountGroup::is_valid("a-1"));
    }

    #[test]
    fn project_groups_obey_ingress_limit() {
        let groups = vec!["group"; crate::shared_values::ACCOUNT_GROUPS_MAX + 1];
        let body = serde_json::json!({"path":"/project", "account_groups":groups});
        assert!(ProjectWrite::deserialize(body).is_err());
    }
}

#[derive(Clone, Copy, Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SidebarAction {
    Reorder,
}

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct SidebarReorder {
    pub action: SidebarAction,
    pub request_id: BoundedText<1, 255>,
    pub expected_revision: SafeInteger,
    pub groups: std::collections::BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub migration: Field<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ProjectRequest {
    Location(LocationWrite),
    Sidebar(SidebarReorder),
    Project(ProjectWrite),
}

impl ProjectRequest {
    pub fn action(&self) -> &'static str {
        match self {
            Self::Project(body) => body.action.unwrap_or(ProjectAction::Register).as_str(),
            Self::Location(body) => match body.action {
                LocationAction::AddLocation => "add_location",
                LocationAction::RemoveLocation => "remove_location",
            },
            Self::Sidebar(_) => "reorder",
        }
    }
    pub fn path(&self) -> Option<&str> {
        match self {
            Self::Project(body) => Some(body.path.as_str()),
            Self::Location(body) => body.path.as_ref().map(BoundedText::as_str),
            Self::Sidebar(_) => None,
        }
    }
    pub fn server(&self) -> Option<&str> {
        match self {
            Self::Project(body) => body.server.as_ref().map(BoundedText::as_str),
            Self::Location(body) => Some(body.server.as_str()),
            Self::Sidebar(_) => None,
        }
    }
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Project(body) => body.request_id.as_ref().map(BoundedText::as_str),
            Self::Location(body) => Some(body.request_id.as_str()),
            Self::Sidebar(body) => Some(body.request_id.as_str()),
        }
    }
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Project(body) => body.name.value().map(String::as_str),
            Self::Location(_) | Self::Sidebar(_) => None,
        }
    }
    pub fn project(&self) -> Option<&str> {
        match self {
            Self::Location(body) => Some(body.project.as_str()),
            Self::Project(_) | Self::Sidebar(_) => None,
        }
    }
}
