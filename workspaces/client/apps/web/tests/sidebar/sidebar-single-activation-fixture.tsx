import type { ServerNavigation } from "../../src/servers/navigation.ts";
import { sidebarRowFixture } from "../../src/servers/sidebarRowFixture.ts";
import { sidebarFixtureRadio } from "./sidebar-fixture-values.ts";
import { snapshotFixture } from "../../src/snapshotFixture.ts";
import type { SidebarChatRow } from "../../src/generated/observableResources.ts";
import type { SidebarSource } from "../../src/servers/mergedSidebar.ts";
declare global {
  interface Window {
    prepared: string;
  }
}
import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import Sidebar from "../../src/components/Sidebar.tsx";
import CombinedServerSidebar from "../../src/servers/CombinedServerSidebar.tsx";
import { mergeSidebar } from "../../src/servers/mergedSidebar.ts";
import { localSidebarBackend } from "../../src/components/sidebar/services.tsx";
import { theme } from "../../src/theme.ts";
import "../../src/style.css";
import "../../src/appearance.css";
const noop = () => {};
const rows = Array.from({ length: 30 }, (_, index) => sidebarRowFixture("chat-" + index, {
  name: "Chat " + index + " with a long title reaching the actions",
  projectPath: "/project",
  groupKey: JSON.stringify(["items", "/project", null]),
  sortKey: String(index).padStart(2, "0"),
  searchText: "chat " + index + " with a long title reaching the actions /project",
}));
const data = {
  ...snapshotFixture([], {
    projects: [{ id: "project", name: "Project", path: "/project", folders: [] }],
    sidebarOrder: { revision: 0, groups: {} },
  }),
  stateDir: "sidebar-single-activation",
  token: "fixture-token",
};
const scenario = new URLSearchParams(location.search).get("groups");
const groupChat = (
  id: string,
  cwd: string,
  archived = false,
): SidebarChatRow => sidebarRowFixture(id, {
  projectPath: cwd,
  archived,
  groupKey: JSON.stringify(["items", cwd, null]),
  searchText: id + " " + cwd,
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
const source: SidebarSource = {
  id: "local",
  online: true,
  sidebar: {
    stateDir: groupData.stateDir,
    rows: groupRows,
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
};
const model = mergeSidebar([source], 0, true);
const backends = new Map([["local", localSidebarBackend]]);
const online = () => true;
// Combined sidebar activation needs only the selected chat; other navigation fields are unused.
const fixtureNavigation = (
  value: Partial<ServerNavigation>,
): ServerNavigation => value as ServerNavigation;
function Fixture() {
  const [opened, setOpened] = useState<string | null>(null);
  const [opens, setOpens] = useState<string[]>([]);
  return (
    <MantineProvider theme={theme} defaultColorScheme="dark">
      {scenario === "combined" ? (
        <CombinedServerSidebar
          model={model}
          backends={backends}
          online={online}
          refreshOwner={async () => fixtureNavigation({})}
          executeCommand={async () => {}}
          aliases={{ local: "Fixture" }}
          current="local"
          navigation={{ local: fixtureNavigation({ opened: null }) }}
          send={noop}
          applyNavigation={noop}
          mobile={true}
          hidden={false}
          close={noop}
          settings={noop}
          search={noop}
          notify={noop}
          notice=""
          hideOldChatsThreshold={2}
        />
      ) : (
        <Sidebar
          data={scenario ? groupData : data}
          opened={opened}
          open={(id) => {
            setOpened(id);
            setOpens((old) => [...old, id]);
          }}
          prepareChat={(id) => {
            window.prepared = id;
          }}
          newChat={noop}
          addProject={noop}
          changeProject={noop}
          projectAccount={noop}
          deleteProject={noop}
          creating={false}
          rename={async () => {}}
          remove={noop}
          mobile={true}
          onSearch={noop}
          close={noop}
          rows={scenario ? groupRows : rows}
          markingRead={new Set()}
          markUnread={noop}
          hideOldChatsThreshold={scenario ? 2 : 100}
        />
      )}
      <output aria-label="Opened chats">{JSON.stringify(opens)}</output>
    </MantineProvider>
  );
}
createRoot(document.getElementById("root")!).render(<Fixture />);
