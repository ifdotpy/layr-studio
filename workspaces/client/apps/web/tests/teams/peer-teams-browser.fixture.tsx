import "../../src/rendererStyles.ts";
import type { ComponentSnapshot as Snapshot } from "../../src/components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { ConversationProvider } from "../../src/components/conversation/support/navigation.ts";
import {
  ServerViewStoreProvider,
  useServerStore,
} from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { resourceData } from "../../src/store/runtime.ts";
import { syncDatabase, defaultSyncClient } from "../../src/sync/client.ts";
import { when } from "mobx";
import { conversationActionsFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import { SidebarCommandsProvider } from "../../src/components/sidebar/support/commands.ts";

import { observer } from "mobx-react-lite";
import { peerTeamRowFixtures } from "./peer-teams-rows.fixture.ts";

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
await fetch("/fixture/publish-state", {
  method: "POST",
  body: JSON.stringify({
    ...initial,
    runtime: {
      ...initial.runtime,
      agentSummaries: Object.values(peerTeamRowFixtures).filter(
        (agent) => agent.id !== "e",
      ),
    },
  }),
});
await syncDatabase();
const release = defaultSyncClient.retainStateDelivery(() => {});
await when(() => defaultSyncClient.projectionMetadata.ready);
window.addEventListener("pagehide", release, { once: true });
const Fixture = observer(function Fixture() {
  const collections = resourceData(useServerStore().state().current);
  const [opened, open] = useState("a");
  const data = initial;
  window.peerTeamsData = {
    ...initial,
    runtime: {
      ...initial.runtime,
      peerTeams: [...(collections?.peerTeams.values() ?? [])].map((team) => ({
        id: team.id,
        name: team.name,
        projectPath: team.projectPath,
        revision: team.revision,
        members: team.members ? [...team.members] : team.members,
      })),
    },
  };
  window.failRefresh = false;
  return (
    <MantineProvider theme={theme} defaultColorScheme="dark">
      <div style={{ width: 320, maxWidth: "100vw" }}>
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
            markUnread: () => {},
          }}
        >
          <Sidebar
            data={data}
            opened={opened}
            creating={false}
            mobile={true}
            close={() => {}}
            {...(collections ? { collections } : {})}
            markingRead={new Set()}
          />
        </SidebarCommandsProvider>
      </div>
      <ConversationProvider value={conversationActionsFixture(open)}>
        <TeamChats leadId={opened} />
      </ConversationProvider>
    </MantineProvider>
  );
});
createRoot(document.getElementById("root")!).render(
  <ServerViewStoreProvider>
    <Fixture />
  </ServerViewStoreProvider>,
);
