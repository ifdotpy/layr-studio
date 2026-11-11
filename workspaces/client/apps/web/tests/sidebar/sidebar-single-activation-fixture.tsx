import "../../src/rendererStyles.ts";
import "@mantine/core/styles.css";
import { sidebarEntities } from "../../src/servers/sidebarSnapshot.ts";
import { SidebarCommandsProvider } from "../../src/components/sidebar/support/commands.ts";
import { observableFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
const injectedStore = observableFixture();
import type { ServerNavigation } from "../../src/servers/navigation.ts";
import { agentSummaryFixture } from "../../src/servers/agentSummaryFixture.ts";
import { sidebarFixtureRadio } from "./sidebar-fixture-values.ts";
import { componentSnapshot as snapshotFixture } from "../../src/components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { createStateMirror } from "../../src/generated/observableResources.ts";
import type { AgentSummaryEntityDto } from "../../src/generated/api.ts";
import {
  agentEntities,
  withSidebarCollections,
} from "../../src/servers/sidebarSourceFixture.ts";
declare global {
  interface Window {
    prepared: string;
  }
}
import { useState } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import Sidebar from "../../src/components/sidebar/Sidebar/Sidebar.fixture.tsx";
import {
  createCombinedSidebarFixture,
  CombinedSidebarFixtureView as CombinedServerSidebar,
} from "../../src/servers/CombinedServerSidebar.fixture.tsx";
import { mergeSidebar } from "../../src/servers/mergedSidebar.ts";
import { localSidebarBackend } from "../../src/components/sidebar/support/services.tsx";
import { theme } from "../../src/theme.ts";
import "../../src/style.css";
import "../../src/appearance.css";
const noop = () => {};
const rows = Array.from({ length: 30 }, (_, index) =>
  agentSummaryFixture("chat-" + index, {
    name: "Chat " + index + " with a long title reaching the actions",
    projectPath: "/project",
    cwd: "/project",
    created: index,
    updated: 1,
  }),
);
const data = {
  ...Object.assign(
    snapshotFixture([], {
      projects: [
        { id: "project", name: "Project", path: "/project", folders: [] },
      ],
      sidebarOrder: { revision: 0, groups: {} },
    }),
    { token: "", stateDir: "/fixture" },
  ),
  stateDir: "sidebar-single-activation",
  token: "fixture-token",
};
const scenario = new URLSearchParams(location.search).get("groups");
const groupChat = (
  id: string,
  cwd: string,
  archived = false,
): AgentSummaryEntityDto =>
  agentSummaryFixture(id, {
    projectPath: cwd,
    cwd,
    archived,
  });
const groupRows = [
  ...Array.from({ length: 5 }, (_, directory) =>
    Array.from({ length: 2 }, (_, chat) =>
      groupChat(
        "archived-" + directory + "-" + chat,
        "/archive-only-" + directory,
        true,
      ),
    ),
  ).flat(),
  groupChat("active-chat", "/active"),
  groupChat("mixed-active", "/mixed"),
  groupChat("mixed-archived", "/mixed", true),
  groupChat("compact-old-0", "/compact"),
  groupChat("compact-old-1", "/compact"),
  groupChat("registered-archived", "/registered-archive", true),
];
const groupData = {
  ...data,
  runtime: {
    ...data.runtime,
    projects: [
      { id: "empty", name: "Registered empty", path: "/empty", folders: [] },
      {
        id: "registered-archive",
        name: "Registered archive",
        path: "/registered-archive",
        folders: [],
      },
    ],
    rooms: [
      {
        id: "shared",
        name: "Shared room",
        projectPath: "/shared-only",
        members: [],
        radio: sidebarFixtureRadio,
      },
    ],
  },
};
const mirror = createStateMirror();
mirror.apply([
  ...rows.map((value) => ({
    collection: "agentSummary" as const,
    id: value.id,
    value,
  })),
  ...data.runtime.projects.map((value) => ({
    collection: "project" as const,
    id: value.id!,
    value,
  })),
]);
const groupMirror = createStateMirror();
groupMirror.apply([
  ...groupData.runtime.rooms.map((value) => ({
    collection: "room" as const,
    id: value.id,
    value,
  })),
  ...groupRows.map((value) => ({
    collection: "agentSummary" as const,
    id: value.id,
    value,
  })),
  ...groupData.runtime.projects.map((value) => ({
    collection: "project" as const,
    id: value.id,
    value,
  })),
]);
for (const [target, snapshot] of [
  [mirror, data],
  [groupMirror, groupData],
] as const) {
  target.apply([
    ...sidebarEntities(target.current),
    {
      collection: "workspace",
      id: "fixture",
      value: {
        ...snapshot.runtime,
        stateDir: snapshot.stateDir,
      },
    },
  ]);
}
const source = withSidebarCollections({
  id: "local",
  online: true,
  sidebar: {
    stateDir: groupData.stateDir,
    entities: agentEntities(groupRows),
    projects: groupData.runtime.projects,
    rooms: groupData.runtime.rooms,
    peerTeams: [],
    sidebarOrder: groupData.runtime.sidebarOrder,
    savedOrder: {},
    compact: {},
    collapsed: {},
    peerTeamsVersion: 1,
    projectOrganizationVersion: 1,
  },
});
const model = mergeSidebar([source], 0, true);
const backends = new Map([["local", localSidebarBackend]]);
const online = () => true;
// Combined sidebar activation needs only the selected chat; other navigation fields are unused.
const fixtureNavigation = (
  value: Partial<ServerNavigation>,
): ServerNavigation => value as ServerNavigation;
const combined = createCombinedSidebarFixture(
  {
    model,
    current: "local",
    navigation: { local: fixtureNavigation({ opened: null }) },
    mobile: true,
    hidden: false,
    close: noop,
    notice: "",
    hideOldChatsThreshold: 2,
  },
  {
    backends,
    online,
    executeCommand: async () => {},
    aliases: { local: "Fixture" },
    send: noop,
    settings: noop,
    search: noop,
    notify: noop,
  },
);
const stopCombined = combined.start();
window.addEventListener("pagehide", stopCombined, { once: true });
function Fixture() {
  const [opened, setOpened] = useState<string | null>(null);
  const [opens, setOpens] = useState<string[]>([]);
  return (
    <MantineProvider theme={theme} defaultColorScheme="dark">
      {scenario === "combined" ? (
        <injectedStore.Provider>
          <CombinedServerSidebar fixture={combined} />
        </injectedStore.Provider>
      ) : (
        <injectedStore.Provider>
          <SidebarCommandsProvider
            value={{
              open: (id) => {
                setOpened(id);
                setOpens((old) => [...old, id]);
              },
              newChat: noop,
              addProject: noop,
              changeProject: noop,
              projectAccount: noop,
              deleteProject: noop,
              rename: async () => {},
              remove: noop,
              onSearch: noop,
              markUnread: noop,
            }}
          >
            <Sidebar
              data={scenario ? groupData : data}
              opened={opened}
              creating={false}
              mobile={true}
              close={noop}
              collections={scenario ? groupMirror.current : mirror.current}
              markingRead={new Set()}
              hideOldChatsThreshold={scenario ? 2 : 100}
            />
          </SidebarCommandsProvider>
        </injectedStore.Provider>
      )}
      <output aria-label="Opened chats">{JSON.stringify(opens)}</output>
    </MantineProvider>
  );
}
createRoot(document.getElementById("root")!).render(<Fixture />);
