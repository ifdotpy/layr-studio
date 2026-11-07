import type { Snapshot } from "../types.ts";
import { snapshotFixture } from "../snapshotFixture.ts";
import { sidebarSnapshot } from "./sidebarSnapshot.ts";
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
    peerTeamsVersion: Snapshot["runtime"]["peerTeamsVersion"];
    projectOrganizationVersion: Snapshot["runtime"]["projectOrganizationVersion"];
    sidebarOrder: Snapshot["runtime"]["sidebarOrder"];
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
    sidebar: sidebarSnapshot(snapshotFixture([]), agentEntities(agents)),
  });
}
export function navigationFromSummaries(
  data: Snapshot,
  agents: readonly SidebarAgent[] = [],
) {
  const source = sidebarFactsFixture(agents);
  return navigationSnapshot(
    data,
    null,
    "",
    source.view.rows,
    [],
    undefined,
    new Set(),
    source.sidebar.entities,
  );
}
