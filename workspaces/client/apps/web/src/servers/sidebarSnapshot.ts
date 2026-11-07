import { saved } from "../api";
import {
  serializeStateCollections,
  type StateCollections,
  type StateEntity,
} from "../generated/observableResources.ts";
import { computed } from "mobx";

export type SidebarEntity = StateEntity;
/** Preserve native facts at the frame serialization boundary. */
export const sidebarEntities = serializeStateCollections;

import type { Room, Snapshot } from "../types";
import { readPreferenceFields } from "../sync/uiPreferenceStore";
import type { PreferenceFields } from "../sync/uiPreferenceMerge";

export type ServerSidebarSnapshot = {
  entities: readonly SidebarEntity[];
  rooms: Room[];
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
  entities: readonly SidebarEntity[] = [],
): ServerSidebarSnapshot {
  const runtime = data.runtime;
  return {
    entities,
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
    studioErrors: runtime?.studioErrors || [],
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

/** Receiver-only computed facade; entity facts never travel twice in the envelope. */
export function sidebarCollectionView(
  sidebar: ServerSidebarSnapshot,
  collections: StateCollections,
) {
  const projects = computed(() => [...collections.projects.values()]);
  const peerTeams = computed(() => [...collections.peerTeams.values()]);
  const workspace = () => collections.workspaces.values().next().value;
  return {
    ...sidebar,
    get projects() {
      return projects.get();
    },
    get peerTeams() {
      return peerTeams.get();
    },
    get stateDir() {
      return workspace()?.stateDir || "";
    },
    get peerTeamsVersion() {
      return workspace()?.peerTeamsVersion;
    },
    get projectOrganizationVersion() {
      return workspace()?.projectOrganizationVersion;
    },
    get sidebarOrder() {
      return workspace()?.sidebarOrder;
    },
  };
}
export type ServerSidebarView = ReturnType<typeof sidebarCollectionView>;
