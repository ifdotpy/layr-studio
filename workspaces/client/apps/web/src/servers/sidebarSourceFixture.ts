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
export function withSidebarCollections(
  source: Omit<SidebarSource, "collections" | "view">,
): SidebarSource {
  const mirror = createStateMirror();
  const entities: readonly SidebarEntity[] = [
    ...source.sidebar.entities,
    ...source.sidebar.projects
      .filter(
        (project) =>
          !source.sidebar.entities.some(
            (entity) =>
              entity.collection === "project" && entity.id === project.id,
          ),
      )
      .map((value) => ({
        collection: "project" as const,
        id: value.id,
        value,
      })),
    ...source.sidebar.peerTeams
      .filter(
        (team) =>
          !source.sidebar.entities.some(
            (entity) =>
              entity.collection === "peerTeam" && entity.id === team.id,
          ),
      )
      .map((value) => ({
        collection: "peerTeam" as const,
        id: value.id,
        value,
      })),
  ];
  mirror.apply(entities);
  return {
    ...source,
    collections: mirror.current,
    view: createSidebarViewModel(
      () => mirror.current,
      () => ({ query: "", archived: false, opened: null, rank: () => 0 }),
    ),
  };
}
export function sourceAgents(source: SidebarSource): readonly SidebarAgent[] {
  return [...source.collections.agents.values()];
}
export function replaceSourceAgents(
  source: SidebarSource,
  agents: readonly SidebarAgent[],
) {
  source.sidebar = {
    ...source.sidebar,
    entities: [
      ...source.sidebar.entities.filter(
        (entity) => entity.collection !== "agentSummary",
      ),
      ...agentEntities(agents),
    ],
  };
  Object.assign(source, withSidebarCollections(source));
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
