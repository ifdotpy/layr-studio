//! Closed stored project records used by project and radio admission.
use crate::persistence::{AdmittedRequest, ProjectRemovalReason, WorkEffectStatus};
use crate::queue_command::Field;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAccountPreference {
    pub account_key: Option<String>,
    #[serde(default)]
    pub account_groups: Vec<String>,
    #[serde(default)]
    pub account_keys: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(tag = "recordKind", rename_all = "lowercase")]
pub enum ProjectRecord {
    Live(Box<ProjectLive>),
    Removal(ProjectTombstone),
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectLive {
    pub id: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<Vec<ProjectPeerTeam>>")]
    pub peer_teams: Field<Vec<ProjectPeerTeam>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub account_key: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<Vec<String>>")]
    pub account_groups: Field<Vec<String>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<Vec<String>>")]
    pub account_keys: Field<Vec<String>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub name: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<Vec<ProjectLocation>>")]
    pub locations: Field<Vec<ProjectLocation>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<Vec<ProjectAlias>>")]
    pub project_aliases: Field<Vec<ProjectAlias>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<u64>")]
    pub locations_revision: Field<u64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<u64>")]
    pub account_revision: Field<u64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<u64>")]
    pub account_groups_revision: Field<u64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub home_server_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<serde_json::Number>")]
    pub created: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<serde_json::Number>")]
    pub updated: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<bool>")]
    pub sidebar_hidden: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<u64>")]
    pub peer_teams_revision: Field<u64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<u64>")]
    pub organization_revision: Field<u64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<Vec<ProjectFolder>>")]
    pub folders: Field<Vec<ProjectFolder>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub worker_base_ref: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<u64>")]
    pub worker_base_revision: Field<u64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub worker_environment: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<u64>")]
    pub worker_environment_revision: Field<u64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub workspace_policy: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<u64>")]
    pub workspace_policy_revision: Field<u64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<BTreeMap<String, ProjectLocationIntent>>")]
    pub location_intents: Field<BTreeMap<String, ProjectLocationIntent>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<ProjectRegistration>")]
    pub registration: Field<ProjectRegistration>,
}
impl ProjectLive {
    pub fn preference(&self) -> ProjectAccountPreference {
        ProjectAccountPreference {
            account_key: self.account_key.value().cloned(),
            account_groups: self.account_groups.value().cloned().unwrap_or_default(),
            account_keys: self.account_keys.value().cloned().unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTombstone {
    pub id: String,
    pub removal: ProjectRemoval,
    #[serde(default)]
    pub peer_teams: Vec<ProjectPeerTeam>,
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRemoval {
    pub reason: ProjectRemovalReason,
    pub sequence: i64,
    pub request_id: String,
    pub deleted_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectLocation {
    pub server_id: String,
    pub path: String,
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub git_origin: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub requested_path: Field<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectAlias {
    pub server_id: String,
    pub project_id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectPeerTeam {
    pub id: String,
    pub name: String,
    pub members: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectFolder {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub parent_id: Field<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectLocationIntent {
    pub request: AdmittedRequest,
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRegistration {
    pub server: String,
    pub path: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub signature: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub request_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<WorkEffectStatus>")]
    pub status: Field<WorkEffectStatus>,
}

pub(crate) fn schema() -> serde_json::Value {
    schemars::schema_for!(ProjectRecord).to_value()
}
pub(crate) fn check_schema_parity() -> Result<(), String> {
    let stored: serde_json::Value = serde_json::from_slice(include_bytes!(
        "schemas/persisted-project-record.schema.json"
    ))
    .map_err(|e| e.to_string())?;
    if stored != schema() {
        return Err(
            "persisted project schema must be generated from project_record::ProjectRecord".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_live_preserves_field_presence() {
        for raw in [
            r#"{"recordKind":"live","id":"p","path":"/p"}"#,
            r#"{"recordKind":"live","id":"p","path":"/p","name":null,"peerTeams":null,"locations":null}"#,
            r#"{"recordKind":"live","id":"p","path":"/p","name":"","peerTeams":[],"locations":[{"serverId":"s","path":"/p","projectId":"p","gitOrigin":null}]}"#,
        ] {
            let expected: serde_json::Value = serde_json::from_str(raw).unwrap();
            let record: ProjectRecord = serde_json::from_str(raw).unwrap();
            assert_eq!(serde_json::to_value(record).unwrap(), expected);
        }
    }

    #[test]
    fn peer_teams_revision_preserves_missing_and_null() {
        for (raw, expected) in [
            (
                r#"{"recordKind":"live","id":"p","path":"/p"}"#,
                Field::Missing,
            ),
            (
                r#"{"recordKind":"live","id":"p","path":"/p","peerTeamsRevision":null}"#,
                Field::Null,
            ),
            (
                r#"{"recordKind":"live","id":"p","path":"/p","peerTeamsRevision":3}"#,
                Field::Value(3),
            ),
        ] {
            let ProjectRecord::Live(project) = serde_json::from_str(raw).unwrap() else {
                panic!("live")
            };
            assert_eq!(project.peer_teams_revision, expected);
            let encoded = serde_json::to_value(ProjectRecord::Live(project)).unwrap();
            assert_eq!(
                encoded.get("peerTeamsRevision"),
                serde_json::from_str::<serde_json::Value>(raw)
                    .unwrap()
                    .get("peerTeamsRevision")
            );
        }
        check_schema_parity().unwrap();
    }
}
