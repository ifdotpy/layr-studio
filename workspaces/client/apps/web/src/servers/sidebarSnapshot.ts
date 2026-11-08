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

import type { components } from "../generated/api.ts";
import { readPreferenceFields } from "../sync/uiPreferenceStore";
import type { PreferenceFields } from "../sync/uiPreferenceMerge";

export type ServerSidebarSnapshot = {
  entities: readonly SidebarEntity[];
  savedOrder: Record<string, string[]>;
  compact: Record<string, boolean>;
  collapsed: Record<string, boolean>;
  /** Edit times of the compact and collapse values, for last-writer-wins merges. */
  visualFields?: PreferenceFields;
  /** Local errors keep their server-owned ids for safe actions. */
  studioErrors?: readonly import("../deepReadonly.ts").DeepReadonly<
    components["schemas"]["StudioError"]
  >[];
};

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
  data: StateCollections,
  entities: readonly SidebarEntity[] = [],
): ServerSidebarSnapshot {
  const stateDir = data.workspaces.values().next().value?.stateDir ?? "";
  return {
    entities,
    studioErrors: [...data.studioErrors.values()],
    savedOrder: saved<Record<string, string[]>>(
      `codex-sidebar-order:${stateDir}`,
      {},
    ),
    compact: saved<Record<string, boolean>>(
      `codex-project-compact:${stateDir}`,
      {},
    ),
    collapsed: saved<Record<string, boolean>>(
      `codex-project-tree:${stateDir}`,
      {},
    ),
    visualFields: sidebarVisualFields(stateDir),
  };
}

/** Receiver-only computed facade; entity facts never travel twice in the envelope. */
export function sidebarCollectionView(
  sidebar: ServerSidebarSnapshot,
  collections: StateCollections,
) {
  const projects = computed(() => [...collections.projects.values()]);
  const peerTeams = computed(() => [...collections.peerTeams.values()]);
  const rooms = computed(() => [...collections.rooms.values()]);
  const workspace = () => collections.workspaces.values().next().value;
  return {
    ...sidebar,
    get projects() {
      return projects.get();
    },
    get peerTeams() {
      return peerTeams.get();
    },
    get rooms() {
      return rooms.get();
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
