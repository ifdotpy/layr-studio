import {
  sidebarCollectionView,
  type ServerSidebarSnapshot,
  type ServerSidebarView,
} from "./sidebarSnapshot.ts";
import type { ComponentSnapshot as Snapshot } from "../components/componentTestFixtures.ts";
import { agentSummaryFixture } from "./agentSummaryFixture.ts";

import type { components } from "../generated/api.ts";
import { navigationSnapshot } from "./navigation.ts";
import { createStateMirror } from "../generated/observableResources.ts";
import {
  createSidebarViewModel,
  type SidebarAgent,
} from "../components/sidebar/viewModel.ts";
import type { SidebarSource } from "./mergedSidebar.ts";
import type { SidebarEntity } from "./sidebarSnapshot.ts";

export function agentEntities(
  agents: readonly SidebarAgent[],
): SidebarEntity[] {
  return agents.map((value) => ({
    collection: "agentSummary",
    id: value.id,
    value,
  }));
}
type SidebarFixtureState =
  import("./sidebarSnapshot.ts").ServerSidebarSnapshot & {
    stateDir: string;
    projects: import("../types.ts").Project[];
    peerTeams: import("../types.ts").PeerTeam[];
    rooms: components["schemas"]["RoomEntityDto"][];
    peerTeamsVersion: components["schemas"]["WorkspaceEntityDto"]["peerTeamsVersion"];
    projectOrganizationVersion: components["schemas"]["WorkspaceEntityDto"]["projectOrganizationVersion"];
    sidebarOrder: components["schemas"]["WorkspaceEntityDto"]["sidebarOrder"];
  };
export function withSidebarCollections(source: {
  id: string;
  online: boolean;
  sidebar: import("./sidebarSnapshot.ts").ServerSidebarSnapshot &
    Partial<SidebarFixtureState>;
}) {
  const mirror = createStateMirror();
  const sidebar: SidebarFixtureState = {
    stateDir: "",
    projects: [],
    peerTeams: [],
    rooms: [],
    peerTeamsVersion: undefined,
    projectOrganizationVersion: undefined,
    sidebarOrder: undefined,
    ...source.sidebar,
  };
  const read = () => {
    mirror.apply([
      ...sidebar.entities.filter(
        (entity) =>
          !["project", "peerTeam", "workspace"].includes(entity.collection),
      ),
      ...(sidebar.rooms || []).map((value) => ({
        collection: "room" as const,
        id: value.id,
        value,
      })),
      ...sidebar.projects.map((value) => ({
        collection: "project" as const,
        id: value.id,
        value,
      })),
      ...sidebar.peerTeams.map((value) => ({
        collection: "peerTeam" as const,
        id: value.id,
        value,
      })),
      {
        collection: "workspace",
        id: "fixture",
        value: {
          stateDir: sidebar.stateDir,
          sidebarOrder: sidebar.sidebarOrder,
          peerTeamsVersion: sidebar.peerTeamsVersion,
          projectOrganizationVersion: sidebar.projectOrganizationVersion,
        },
      },
    ]);
    return mirror.current;
  };
  return {
    ...source,
    sidebar,
    get collections() {
      return read();
    },
    view: createSidebarViewModel(read, () => ({
      query: "",
      archived: false,
      opened: null,
      rank: () => 0,
    })),
  };
}
export function sourceAgents(source: SidebarSource): readonly SidebarAgent[] {
  return [...source.collections.agents.values()];
}
export function replaceSourceAgents(
  source: ReturnType<typeof withSidebarCollections>,
  agents: readonly SidebarAgent[],
) {
  source.sidebar.entities = [
    ...source.sidebar.entities.filter(
      (entity) => entity.collection !== "agentSummary",
    ),
    ...agentEntities(agents),
  ];
}

export function sidebarFactsFixture(agents: readonly SidebarAgent[]) {
  return withSidebarCollections({
    id: "local",
    online: true,
    sidebar: {
      entities: agentEntities(agents),
      savedOrder: {},
      compact: {},
      collapsed: {},
      studioErrors: [],
    },
  });
}
export function navigationFromSummaries(
  data: Snapshot,
  agents: readonly SidebarAgent[] = [],
) {
  const source = sidebarFactsFixture(agents);
  return navigationSnapshot(
    stateFromFixture(data),
    null,
    "",
    source.view.rows,
    [],
    undefined,
    new Set(),
    source.sidebar.entities,
    {
      workspaceId: data.entityWorkspaceId ?? "",
      ready: true,
      complete: data.entitiesComplete !== false,
      projectSequences: {},
    },
  );
}

/** Legacy scenario inputs seed real generated models; production never calls this helper. */
export function stateFromFixture(
  data: Snapshot,
): import("../generated/observableResources.ts").StateCollections;
export function stateFromFixture(data: null | undefined): undefined;
export function stateFromFixture(data: Snapshot | null | undefined) {
  if (!data) return undefined;
  const mirror = createStateMirror();
  mirror.apply([
    ...data.threads.map((agent) => ({
      collection: "agentSummary" as const,
      id: agent.id,
      value: {
        ...agentSummaryFixture(agent.id),
        ...agent,
        inFlight: agent.inFlight === true,
        projectServerId: agent.projectServerId ?? null,
        markRead: agent.markRead ?? null,
      },
    })),
    ...data.chats.map((value) => ({
      collection: "chat" as const,
      id: value.id,
      value,
    })),
    ...data.runtime.projects.map((value) => ({
      collection: "project" as const,
      id: value.id,
      value,
    })),
    ...data.runtime.rooms.map((value) => ({
      collection: "room" as const,
      id: value.id,
      value,
    })),
    ...data.runtime.peerTeams.map((value) => ({
      collection: "peerTeam" as const,
      id: value.id,
      value,
    })),
    ...data.runtime.studioErrors.map((value) => ({
      collection: "studioError" as const,
      id: value.id,
      value,
    })),
    {
      collection: "workspace",
      id: "current",
      value: { ...data.runtime, stateDir: data.stateDir },
    },
  ]);
  return mirror.current;
}

const membershipInputs = {
  query: "",
  archived: false,
  opened: null,
  rank: () => 0,
};
export function createSidebarSourceStore() {
  const snapshots = new Map<
    string,
    {
      sidebar: ServerSidebarView;
      mirror: ReturnType<typeof createStateMirror>;
      view: ReturnType<typeof createSidebarViewModel>;
    }
  >();
  return {
    update(id: string, sidebar: ServerSidebarSnapshot | undefined) {
      if (!sidebar) return;
      const previous = snapshots.get(id);
      const mirror = previous?.mirror || createStateMirror();
      const view =
        previous?.view ||
        createSidebarViewModel(
          () => mirror.current,
          () => membershipInputs,
        );
      mirror.apply(sidebar.entities);
      snapshots.set(id, {
        sidebar: sidebarCollectionView(sidebar, mirror.current),
        mirror,
        view,
      });
    },
    forget(id: string) {
      snapshots.delete(id);
    },
    collections(id: string) {
      return snapshots.get(id)?.mirror.current;
    },
    sources(
      ids: readonly string[],
      online: ReadonlySet<string>,
      creationFor?: (owner: string) => SidebarSource["creation"],
    ): SidebarSource[] {
      return ids.flatMap((id) => {
        const value = snapshots.get(id);
        return value
          ? [
              {
                id,
                creation: creationFor?.(id),
                sidebar: value.sidebar,
                collections: value.mirror.current,
                view: value.view,
                online: online.has(id),
              },
            ]
          : [];
      });
    },
  };
}
