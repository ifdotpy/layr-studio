import "../../src/rendererStyles.ts";
import type { ComponentSnapshot as Snapshot } from "../../src/components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { messageModels } from "../messages/message-models.fixture";
import { conversationActionsFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import { SidebarCommandsProvider } from "../../src/components/sidebar/support/commands.ts";
import { observableFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
const injectedStore = observableFixture();
import { observer } from "mobx-react-lite";
import { peerTeamRowFixtures } from "./peer-teams-rows.fixture.ts";
import {
  createStateMirror,
  serializeStateCollections,
} from "../../src/generated/observableResources.ts";
import type { components } from "../../src/generated/api.ts";
import type { Agent } from "../../src/types.ts";
import { fixtureSnapshot } from "../fixtures/snapshot.ts";
type PeerProject = Pick<
  components["schemas"]["ProjectEntityDto"],
  "id" | "name" | "path" | "peerTeamsRevision"
> & {
  folders?:
    | Pick<components["schemas"]["ProjectFolder"], "id" | "name" | "parentId">[]
    | null
    | undefined;
};
/** The fixture serializes project folder identity fields, without open JSON extensions. */
export type PeerSidebarEntities = (
  | Extract<
      components["schemas"]["SyncEntityPayload"],
      { collection: "agentSummary" | "peerTeam" }
    >
  | (Omit<
      Extract<
        components["schemas"]["SyncEntityPayload"],
        { collection: "project" }
      >,
      "value"
    > & { value: PeerProject })
)[];
declare global {
  interface Window {
    setFixture: React.Dispatch<React.SetStateAction<Snapshot>>;
    // Renamed from fixture: performance/scroll-restart.fixture.tsx declares scroll fixture controls.
    peerTeamsData: Snapshot;
    failRefresh: boolean;
    receivePeerSidebar: (dto: PeerSidebarEntities) => void;
  }
}

import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import Sidebar from "../../src/components/sidebar/Sidebar/Sidebar.fixture.tsx";
import TeamChats from "../../src/components/shell/messages/TeamChats/TeamChats.tsx";
import "../../src/style.css";
import "../../src/workspace-layout.css";
import "../../src/appearance.css";
import { theme } from "../../src/theme.ts";
const a = (
  id: string,
  name: string,
  cwd = "/p",
  extra: Partial<Agent> = {},
): Agent => ({
  id,
  name,
  cwd,
  source: "managed",
  isLead: true,
  created: 1,
  updated: 1,
  ...extra,
});
const initial: Snapshot = {
  ...fixtureSnapshot([]),
  stateDir: "fixture",
  threads: [
    a("a", "Alpha"),
    a("b", "Beta", "/p", { projectFolder: "f" }),
    a("c", "Gamma"),
    a("x", "Other project", "/other"),
    a("w", "Worker", "/p", { isLead: false, rootId: "a" }),
    a("i", "Imported", "/p", {
      // @ts-expect-error source deliberately uses the obsolete imported value to exclude non-managed chats.
      source: "imported",
    }),
    a("d", "Deleted", "/p", { deletedAt: 1 }),
  ],
  runtime: {
    ...fixtureSnapshot([]).runtime,
    peerTeamsVersion: 1,
    peerTeams: [],
    projects: [
      {
        id: "p",
        path: "/p",
        name: "Project",
        peerTeamsRevision: 0,
        folders: [{ id: "f", name: "Folder", parentId: null }],
      },
    ],
    rooms: [],
    requests: [],
    complaints: [],
    agents: [],
  },
};
const sidebar = createStateMirror();
let currentSnapshot = initial;
const receiveSidebar = (dto: Parameters<typeof sidebar.apply>[0]) =>
  sidebar.apply([
    ...dto.filter(
      (item) => item.collection !== "workspace" && item.collection !== "room",
    ),
    {
      collection: "workspace",
      id: "fixture",
      value: { ...currentSnapshot.runtime, stateDir: currentSnapshot.stateDir },
    },
    ...currentSnapshot.runtime.rooms.map((value) => ({
      collection: "room" as const,
      id: value.id,
      value,
    })),
  ]);
receiveSidebar([
  ...[
    peerTeamRowFixtures.a,
    peerTeamRowFixtures.b,
    peerTeamRowFixtures.c,
    peerTeamRowFixtures.x,
  ].map((value) => ({
    collection: "agentSummary" as const,
    id: value.id,
    value,
  })),
  ...initial.runtime.projects.map((value) => ({
    collection: "project" as const,
    id: value.id!,
    value,
  })),
]);
window.receivePeerSidebar = receiveSidebar;
const models = messageModels(initial.stateDir, initial.threads);
const Fixture = observer(function Fixture() {
  const [data, setData] = useState(initial);
  const [opened, open] = useState("a");
  window.setFixture = (value) => {
    const next = typeof value === "function" ? value(currentSnapshot) : value;
    currentSnapshot = next;
    receiveSidebar(serializeStateCollections(sidebar.current));
    setData(next);
  };
  window.peerTeamsData = data;
  window.failRefresh = false;
  const refresh = async () => {
    if (window.failRefresh) throw Error("Refresh failed");
    const r = await fetch("/snapshot");
    window.setFixture(await r.json());
  };
  models.apply([
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
  ]);
  return (
    <MantineProvider theme={theme} defaultColorScheme="dark">
      <div style={{ width: 320, maxWidth: "100vw" }}>
        <injectedStore.Provider>
          <SidebarCommandsProvider
            value={{
              open: open,
              newChat: () => {},
              addProject: () => {},
              changeProject: () => {},
              projectAccount: () => {},
              rename: async () => {},
              remove: () => {},
              onSearch: () => {},
              refresh: refresh,
              markUnread: () => {},
            }}
          >
            <Sidebar
              data={data}
              opened={opened}
              creating={false}
              mobile={true}
              close={() => {}}
              collections={sidebar.current}
              markingRead={new Set()}
            />
          </SidebarCommandsProvider>
        </injectedStore.Provider>
      </div>
      <models.Provider
        commands={Object.assign(conversationActionsFixture(open), { refresh })}
      >
        <TeamChats leadId={opened} />
      </models.Provider>
    </MantineProvider>
  );
});
createRoot(document.getElementById("root")!).render(<Fixture />);
