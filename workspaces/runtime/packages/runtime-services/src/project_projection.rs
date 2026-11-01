//! Typed public project sync view.
use crate::sync_views::{SYNC_ARRAY_ITEMS, bound_sync_text};
use serde::Serialize;
use studio_runtime_schema::project_projection::{
    ProjectReadiness, ProjectSync, ProjectVault, ReadinessState,
};
use studio_runtime_schema::project_record::{
    ProjectAlias, ProjectFolder, ProjectLive, ProjectLocation, ProjectPeerTeam,
};
use studio_runtime_schema::queue_command::Field;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProjection {
    id: String,
    path: String,
    #[serde(skip_serializing_if = "Field::is_missing")]
    peer_teams: Field<Vec<ProjectPeerTeam>>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    account_key: Field<String>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    account_groups: Field<Vec<String>>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    account_keys: Field<Vec<String>>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    name: Field<String>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    locations: Field<Vec<ProjectLocation>>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    project_aliases: Field<Vec<ProjectAlias>>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    locations_revision: Field<u64>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    account_revision: Field<u64>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    account_groups_revision: Field<u64>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    home_server_id: Field<String>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    created: Field<serde_json::Number>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    updated: Field<serde_json::Number>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    peer_teams_revision: Field<u64>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    organization_revision: Field<u64>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    folders: Field<Vec<ProjectFolder>>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    worker_base_ref: Field<String>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    worker_base_revision: Field<u64>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    worker_environment: Field<String>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    worker_environment_revision: Field<u64>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    layr_readiness: Field<ProjectReadiness>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    layr_sync: Field<ProjectSync>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    vault: Field<ProjectVault>,
}

fn text(field: &mut Field<String>, name: &str) {
    if let Field::Value(value) = field {
        *value = bound_sync_text(value, name);
    }
}
fn strings(field: &mut Field<Vec<String>>) {
    if let Field::Value(values) = field {
        values.truncate(SYNC_ARRAY_ITEMS);
        for value in values {
            *value = bound_sync_text(value, "");
        }
    }
}

pub fn project_record(
    mut project: ProjectLive,
    mut readiness: Field<ProjectReadiness>,
    mut sync: Field<ProjectSync>,
    mut vault: Field<ProjectVault>,
) -> Option<ProjectProjection> {
    if matches!(readiness, Field::Null) {
        readiness = Field::Missing;
    }
    if project.id.is_empty() {
        return None;
    }
    project.id = bound_sync_text(&project.id, "id");
    project.path = bound_sync_text(&project.path, "path");
    for (value, name) in [
        (&mut project.account_key, "accountKey"),
        (&mut project.name, "name"),
        (&mut project.home_server_id, "homeServerId"),
        (&mut project.worker_base_ref, "workerBaseRef"),
        (&mut project.worker_environment, "workerEnvironment"),
    ] {
        text(value, name);
    }
    strings(&mut project.account_groups);
    strings(&mut project.account_keys);
    if let Field::Value(teams) = &mut project.peer_teams {
        teams.truncate(SYNC_ARRAY_ITEMS);
        for team in teams {
            team.id = bound_sync_text(&team.id, "id");
            team.name = bound_sync_text(&team.name, "name");
            team.members.truncate(SYNC_ARRAY_ITEMS);
            for member in &mut team.members {
                *member = bound_sync_text(member, "");
            }
        }
    }
    if let Field::Value(locations) = &mut project.locations {
        locations.truncate(SYNC_ARRAY_ITEMS);
        for location in locations {
            location.server_id = bound_sync_text(&location.server_id, "serverId");
            location.path = bound_sync_text(&location.path, "path");
            location.project_id = bound_sync_text(&location.project_id, "projectId");
            text(&mut location.git_origin, "gitOrigin");
            text(&mut location.requested_path, "requestedPath");
        }
    }
    if let Field::Value(aliases) = &mut project.project_aliases {
        aliases.truncate(SYNC_ARRAY_ITEMS);
        for alias in aliases {
            alias.server_id = bound_sync_text(&alias.server_id, "serverId");
            alias.project_id = bound_sync_text(&alias.project_id, "projectId");
            alias.name = bound_sync_text(&alias.name, "name");
        }
    }
    if let Field::Value(folders) = &mut project.folders {
        folders.truncate(SYNC_ARRAY_ITEMS);
        for folder in folders {
            folder.id = bound_sync_text(&folder.id, "id");
            folder.name = bound_sync_text(&folder.name, "name");
            text(&mut folder.parent_id, "parentId");
        }
    }
    if let Field::Value(value) = &mut readiness {
        text(&mut value.project_id, "projectId");
        text(&mut value.error, "error");
    }
    if !matches!(&readiness, Field::Value(value) if value.state == ReadinessState::Ready) {
        sync = Field::Missing;
        vault = Field::Missing;
    }
    if let Field::Value(value) = &mut sync {
        value.project_id = bound_sync_text(&value.project_id, "projectId");
        for values in [
            &mut value.conflicts,
            &mut value.choices,
            &mut value.with_agent,
        ] {
            values.truncate(SYNC_ARRAY_ITEMS);
            for item in values {
                *item = bound_sync_text(item, "");
            }
        }
        text(&mut value.reason, "reason");
    }
    if let Field::Value(value) = &mut vault {
        value.project_id = bound_sync_text(&value.project_id, "projectId");
        for (field, name) in [
            (&mut value.disk, "disk"),
            (&mut value.prefix, "prefix"),
            (&mut value.reason, "reason"),
            (&mut value.site, "site"),
        ] {
            text(field, name);
        }
    }
    Some(ProjectProjection {
        id: project.id,
        path: project.path,
        peer_teams: project.peer_teams,
        account_key: project.account_key,
        account_groups: project.account_groups,
        account_keys: project.account_keys,
        name: project.name,
        locations: project.locations,
        project_aliases: project.project_aliases,
        locations_revision: project.locations_revision,
        account_revision: project.account_revision,
        account_groups_revision: project.account_groups_revision,
        home_server_id: project.home_server_id,
        created: project.created,
        updated: project.updated,
        peer_teams_revision: project.peer_teams_revision,
        organization_revision: project.organization_revision,
        folders: project.folders,
        worker_base_ref: project.worker_base_ref,
        worker_base_revision: project.worker_base_revision,
        worker_environment: project.worker_environment,
        worker_environment_revision: project.worker_environment_revision,
        layr_readiness: readiness,
        layr_sync: sync,
        vault,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use studio_runtime_schema::project_record::ProjectRecord;

    #[test]
    fn project_view_preserves_presence_and_bounds_known_fields() {
        let ProjectRecord::Live(project) = serde_json::from_str(r#"{"recordKind":"live","id":"p","path":"/p","name":null,"peerTeams":[],"locations":[],"sidebarHidden":false,"workspacePolicy":"isolated"}"#).unwrap() else { panic!("live") };
        let mut project = *project;
        project.account_groups = Field::Value(vec!["x".repeat(12_001); 201]);
        let projection =
            project_record(project, Field::Null, Field::Missing, Field::Missing).unwrap();
        let encoded = serde_json::to_string(&projection).unwrap();
        let actual: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(actual["name"], json!(null));
        assert_eq!(actual["peerTeams"], json!([]));
        assert_eq!(actual["locations"], json!([]));
        assert_eq!(actual["accountGroups"].as_array().unwrap().len(), 200);
        assert_eq!(
            actual["accountGroups"][0].as_str().unwrap().chars().count(),
            12_000
        );
        assert!(actual.get("layrReadiness").is_none());
        assert!(actual.get("sidebarHidden").is_none());
        assert!(actual.get("workspacePolicy").is_none());
    }
}
