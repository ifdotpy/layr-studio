import { saved } from "../api";
import type { StateCollections } from "../generated/observableResources.ts";
import type { components } from "../generated/api.ts";
import type { DeepReadonly } from "../deepReadonly.ts";
import { toJS } from "mobx";

export type SidebarEntity = Extract<
  components["schemas"]["SyncEntityPayload"],
  {
    collection:
      | "agentSummary"
      | "project"
      | "peerTeam"
      | "workspace"
      | "agentTail";
  }
>;

/** Preserve native collection identities at the frame serialization boundary. */
export function sidebarEntities(
  collections: StateCollections,
): DeepReadonly<SidebarEntity[]> {
  return [
    ...[...collections.agents].map(([id, value]) => ({
      collection: "agentSummary" as const,
      id,
      value: toJS(value),
    })),
    ...[...collections.projects].map(([id, value]) => ({
      collection: "project" as const,
      id,
      value: toJS(value),
    })),
    ...[...collections.peerTeams].map(([id, value]) => ({
      collection: "peerTeam" as const,
      id,
      value: toJS(value),
    })),
    ...[...collections.workspaces].map(([id, value]) => ({
      collection: "workspace" as const,
      id,
      value: toJS(value),
    })),
    ...[...collections.agentTail].map(([id, value]) => ({
      collection: "agentTail" as const,
      id,
      value: toJS(value),
    })),
  ];
}
import type { PeerTeam, Project, Room, Snapshot } from "../types";
import { readPreferenceFields } from "../sync/uiPreferenceStore";
import type { PreferenceFields } from "../sync/uiPreferenceMerge";

export type ServerSidebarSnapshot = {
  stateDir: string;
  entities: DeepReadonly<SidebarEntity[]>;
  projects: Project[];
  rooms: Room[];
  peerTeams: PeerTeam[];
  peerTeamsVersion: Snapshot["runtime"]["peerTeamsVersion"];
  projectOrganizationVersion: Snapshot["runtime"]["projectOrganizationVersion"];
  sidebarOrder: Snapshot["runtime"]["sidebarOrder"];
  savedOrder: Record<string, string[]>;
  compact: Record<string, boolean>;
  collapsed: Record<string, boolean>;
  /** Edit times of the compact and collapse values, for last-writer-wins merges. */
  visualFields?: PreferenceFields;
  /** Local errors keep their server-owned ids for safe actions. */
  studioErrors?: Snapshot["runtime"]["studioErrors"];
};

function pick<T extends object, K extends keyof T>(
  value: T,
  keys: readonly K[],
): Pick<T, K> {
  const result = {} as Pick<T, K>;
  for (const key of keys)
    if (Object.hasOwn(value, key)) result[key] = value[key];
  return result;
}

/** The saved preference fields of this server's compact and collapse records. */
export function sidebarVisualFields(
  stateDir: string,
  fields: PreferenceFields = readPreferenceFields("server"),
): PreferenceFields {
  const keys = new Set([
    `codex-project-compact:${stateDir}`,
    `codex-project-tree:${stateDir}`,
  ]);
  const result: PreferenceFields = {};
  for (const [name, field] of Object.entries(fields)) {
    try {
      const [key] = JSON.parse(name);
      if (keys.has(key)) result[name] = field;
    } catch {
      /* A field name that is not JSON is not a sidebar value. */
    }
  }
  return result;
}

/** Preserve sidebar fields and source identities without transcripts or session state. */
export function sidebarSnapshot(
  data: Snapshot,
  entities: DeepReadonly<SidebarEntity[]> = [],
): ServerSidebarSnapshot {
  const runtime = data.runtime;
  return {
    stateDir: data.stateDir,
    entities,
    projects: (runtime?.projects || []).map((project) =>
      pick(project, [
        "id",
        "path",
        "name",
        "folders",
        "organizationRevision",
        "peerTeams",
        "peerTeamsRevision",
        "homeServerId",
        "locations",
        "locationsRevision",
        "projectAliases",
        "accountKey",
        "accountKeys",
        "accountRevision",
        "accountGroups",
        "accountGroupsRevision",
        "created",
        "updated",
      ]),
    ),
    rooms: (runtime?.rooms || []).map((room) =>
      pick(room, [
        "id",
        "kind",
        "name",
        "projectPath",
        "members",
        "localMembers",
        "rootId",
        "peerTeamId",
        "peerTeamName",
        "peerLabel",
        "userHidden",
        "radio",
        "updated",
      ]),
    ),
    peerTeams: (runtime?.peerTeams || []).map((team) =>
      pick(team, ["id", "name", "projectPath", "members", "revision"]),
    ),
    studioErrors: runtime?.studioErrors || [],
    peerTeamsVersion: runtime?.peerTeamsVersion,
    projectOrganizationVersion: runtime?.projectOrganizationVersion,
    sidebarOrder: runtime?.sidebarOrder,
    savedOrder: saved<Record<string, string[]>>(
      `codex-sidebar-order:${data.stateDir}`,
      {},
    ),
    compact: saved<Record<string, boolean>>(
      `codex-project-compact:${data.stateDir}`,
      {},
    ),
    collapsed: saved<Record<string, boolean>>(
      `codex-project-tree:${data.stateDir}`,
      {},
    ),
    visualFields: sidebarVisualFields(data.stateDir),
  };
}
