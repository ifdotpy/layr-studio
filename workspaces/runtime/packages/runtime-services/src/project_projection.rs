//! Typed public project sync view.
use crate::sync_views::{SYNC_ARRAY_ITEMS, bound_sync_text};
use serde::Serialize;
use std::collections::BTreeSet;
use studio_runtime_schema::project_projection::{
    ProjectReadiness, ProjectSync, ProjectVault, ReadinessState,
};
use studio_runtime_schema::project_record::{
    PeerLead, ProjectAlias, ProjectFolder, ProjectLive, ProjectLocation, ProjectPeerTeam,
};
use studio_runtime_schema::queue_command::Field;
use studio_runtime_store::StoreResult;
use studio_runtime_store::project_grants::{self, GrantRow};

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
    created: Field<studio_json::Number>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    updated: Field<studio_json::Number>,
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

/// The peer leads of one local project: one per active grant of it. The label comes from
/// the grant, so no paired server row is needed. A revoked grant gives none (decision
/// 2026-11-01, "Leads in one team").
pub fn active_peer_leads(project_id: &str, grants: &[GrantRow]) -> Vec<PeerLead> {
    let mut seen = BTreeSet::new();
    grants
        .iter()
        .filter(|row| row.local_project == project_id && row.revoked_at.is_none())
        .filter(|row| seen.insert(row.grant_id.clone()))
        .map(|row| PeerLead {
            grant: row.grant_id.clone(),
            remote_project: row.remote_project.clone(),
            server: row.peer.clone(),
            label: row.peer_label.clone(),
        })
        .collect()
}

/// The peer leads of a local project, read from the store. Used by every writer of a
/// peer team, so the team entities and the project record agree.
pub fn peer_leads_of(
    connection: &rusqlite::Connection,
    project_id: &str,
) -> StoreResult<Vec<PeerLead>> {
    let grants = project_grants::all_rows(connection)?;
    Ok(active_peer_leads(project_id, &grants))
}

/// The `project` document of local project `id`, built at read time from its stored
/// row `raw` and its grants. None when the project is removed or hidden from the
/// sidebar, or when its row does not decode (the reader skips it and reports it).
pub fn local_project_view(
    connection: &rusqlite::Connection,
    id: &str,
    raw: &str,
) -> StoreResult<Option<studio_json::OwnedRawJson>> {
    let record: studio_runtime_schema::project_record::ProjectRecord =
        match studio_json::from_str(raw) {
            Ok(record) => record,
            Err(error) => {
                eprintln!("ERROR project projection skipped row id={id}: {error}");
                return Ok(None);
            }
        };
    let studio_runtime_schema::project_record::ProjectRecord::Live(project) = record else {
        return Ok(None);
    };
    if project.sidebar_hidden.value() == Some(&true) {
        return Ok(None);
    }
    let peer_leads = peer_leads_of(connection, &project.id)?;
    project_record(
        *project,
        Field::Missing,
        Field::Missing,
        Field::Missing,
        &peer_leads,
    )
    .map(|projection| {
        // An empty team list is left out, as in the stored record.
        let mut view = studio_json::to_value(&projection)?;
        if let Some(fields) = view.as_object_mut()
            && fields
                .get("peerTeams")
                .is_some_and(|teams| teams.as_array().is_some_and(Vec::is_empty))
        {
            fields.remove("peerTeams");
        }
        studio_json::OwnedRawJson::from_string(studio_json::to_string(&view)?)
    })
    .transpose()
    .map_err(Into::into)
}

pub fn project_record(
    mut project: ProjectLive,
    mut readiness: Field<ProjectReadiness>,
    mut sync: Field<ProjectSync>,
    mut vault: Field<ProjectVault>,
    peer_leads: &[PeerLead],
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
            team.peer_leads = peer_leads.to_vec();
        }
    }
    if let Field::Value(locations) = &mut project.locations {
        locations.truncate(SYNC_ARRAY_ITEMS);
        for location in locations {
            location.server_id = bound_sync_text(location.server_id.as_str(), "serverId").into();
            location.path = bound_sync_text(&location.path, "path");
            location.project_id = bound_sync_text(&location.project_id, "projectId");
            text(&mut location.git_origin, "gitOrigin");
            text(&mut location.requested_path, "requestedPath");
        }
    }
    if let Field::Value(aliases) = &mut project.project_aliases {
        aliases.truncate(SYNC_ARRAY_ITEMS);
        for alias in aliases {
            alias.server_id = bound_sync_text(alias.server_id.as_str(), "serverId").into();
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
    use studio_json::json;
    use studio_runtime_schema::project_record::ProjectRecord;

    #[test]
    fn project_view_preserves_presence_and_bounds_known_fields() {
        let ProjectRecord::Live(project) = studio_json::from_str(r#"{"recordKind":"live","id":"p","path":"/p","name":null,"peerTeams":[],"locations":[],"sidebarHidden":false,"workspacePolicy":"isolated"}"#).unwrap() else { panic!("live") };
        let mut project = *project;
        project.account_groups = Field::Value(vec!["x".repeat(12_001); 201]);
        let projection =
            project_record(project, Field::Null, Field::Missing, Field::Missing, &[]).unwrap();
        let encoded = studio_json::to_string(&projection).unwrap();
        let actual: studio_json::Value = studio_json::from_str(&encoded).unwrap();
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

    fn grant(grant_id: &str, capability: &str, revoked_at: Option<i64>) -> GrantRow {
        GrantRow {
            grant_id: grant_id.into(),
            peer: "server-b".into(),
            peer_public_key: "PEM".into(),
            local_project: "proj-a".into(),
            remote_project: "proj-b".into(),
            capability: capability.parse().unwrap(),
            created_at: 10,
            revoked_at,
            peer_label: "Laptop".into(),
        }
    }

    fn team_project() -> ProjectLive {
        let ProjectRecord::Live(project) = studio_json::from_str(r#"{"recordKind":"live","id":"proj-a","path":"/a","peerTeams":[{"id":"t1","name":"Team","members":["lead-a","sub-a"]}],"locations":[]}"#).unwrap() else { panic!("live") };
        *project
    }

    #[test]
    fn active_grant_gives_one_peer_lead_on_each_team() {
        let grants = [grant("g1", "LeadMessages", None)];
        let leads = active_peer_leads("proj-a", &grants);
        assert_eq!(
            leads,
            vec![PeerLead {
                grant: "g1".into(),
                remote_project: "proj-b".into(),
                server: "server-b".into(),
                label: "Laptop".into(),
            }]
        );
        let projection = project_record(
            team_project(),
            Field::Null,
            Field::Missing,
            Field::Missing,
            &leads,
        )
        .unwrap();
        let encoded = studio_json::to_string(&projection).unwrap();
        assert!(
            encoded.contains(
                r#""peerLeads":[{"grant":"g1","remoteProject":"proj-b","server":"server-b","label":"Laptop"}]"#
            ),
            "{encoded}"
        );
    }

    #[test]
    fn revoked_grant_gives_no_peer_lead() {
        let grants = [grant("g1", "LeadMessages", Some(20))];
        assert!(active_peer_leads("proj-a", &grants).is_empty());
        let projection = project_record(
            team_project(),
            Field::Null,
            Field::Missing,
            Field::Missing,
            &[],
        )
        .unwrap();
        let encoded = studio_json::to_string(&projection).unwrap();
        assert!(!encoded.contains("peerLeads"), "{encoded}");
        assert!(
            encoded.contains(r#""members":["lead-a","sub-a"]"#),
            "{encoded}"
        );
    }

    #[test]
    fn other_project_gives_no_peer_lead() {
        let other_project = GrantRow {
            local_project: "proj-z".into(),
            ..grant("g2", "LeadMessages", None)
        };
        assert!(active_peer_leads("proj-a", &[other_project]).is_empty());
    }

    #[test]
    fn two_capabilities_of_one_grant_give_one_peer_lead() {
        let grants = [
            grant("g1", "LeadMessages", None),
            grant("g1", "LeadMessages", None),
        ];
        assert_eq!(active_peer_leads("proj-a", &grants).len(), 1);
    }
}
