import "../../src/rendererStyles.ts";
import type { ComponentSnapshot as Snapshot } from "../../src/components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { agentSummaryFixture } from "../../src/servers/agentSummaryFixture.ts";
import { SidebarCommandsProvider } from "../../src/components/sidebar/support/commands.ts";
import { observableFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
const injectedStore = observableFixture();
declare global {
  interface Window {
    // fixture stores scroll handlers in tests/performance/scroll-restart.fixture.tsx.
    radioCreateSnapshot: Snapshot;
    // opened is SessionActivityItem[] in tests/chat/session-activity-browser.fixture.tsx.
    radioOpened: string | null;
    reopen: () => void;
    failRefresh: boolean;
    ordinary: boolean;
  }
}
const rootElement = document.getElementById("root")!;
import { componentSnapshot as snapshotFixture } from "../../src/components/conversation/ComposerAttachments/componentTestFixtures.ts";

import { radioSnapshot } from "./radio-fixture.ts";

import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider, Modal } from "@mantine/core";
import "@mantine/core/styles.css";
import SharedChatCreate from "../../src/components/projects/SharedChatCreate/SharedChatCreate.tsx";
import Sidebar from "../../src/components/sidebar/Sidebar/Sidebar.fixture.tsx";
import "../../src/style.css";
import "../../src/appearance.css";
import "../../src/workspace-layout.css";
import { theme } from "../../src/theme.ts";
type SharedAccounts = NonNullable<
  React.ComponentProps<typeof SharedChatCreate>["accounts"]
>;
type RadioAccount = Pick<
  SharedAccounts["accounts"][number],
  "id" | "label" | "email" | "provider" | "status"
>;
const accounts: SharedAccounts = {
  archivedAccounts: [],
  defaultAccountGroups: [],
  logins: [],
  supportsDelete: true,
  supportsDisconnect: true,
  defaultAccountKey: "codex",
  accounts: (
    [
      {
        id: "codex",
        label: "Personal",
        email: "personal@example.com",
        provider: "codex",
        status: "ready",
      },
      {
        id: "claude",
        label: "Work",
        email: "work@example.com",
        provider: "claude",
        status: "ready",
      },
      ...Array.from({ length: 5 }, (_, i): RadioAccount => ({
        id: "extra" + i,
        label: "Account " + (i + 3),
        email: "account" + i + "@example.com",
        provider: i < 3 ? "codex" : "claude",
        status: "ready",
      })),
    ] satisfies RadioAccount[]
  ).map((account) => ({
    ...account,
    home: "/fixture/" + account.id,
    source: "fixture",
  })),
};
const initial: Snapshot = {
  ...Object.assign(snapshotFixture(), { token: "", stateDir: "/fixture" }),
  stateDir: "create-test",
  threads: [],
  runtime: {
    ...Object.assign(snapshotFixture(), { token: "", stateDir: "/fixture" })
      .runtime,
    projects: [{ id: "p", path: "/p", name: "Project" }],
    rooms: [],
    requests: [],
    peerTeamsVersion: 1,
    peerTeams: [],
  },
};
window.radioCreateSnapshot = initial;
window.radioOpened = null;
window.reopen = () => {};
window.failRefresh = false;
window.ordinary = false;
function Fixture() {
  const [data, setData] = useState(initial);
  const [show, setShow] = useState<{ path?: string | undefined } | null>({
    path: "/p",
  });
  const [opened, open] = useState<string | null>(null);
  window.radioCreateSnapshot = data;
  window.radioOpened = opened;
  window.reopen = () => setShow({ path: "/p" });
  const refresh = async () => {
    if (window.failRefresh) throw Error("Snapshot unavailable");
    const result = await fetch("/snapshot");
    setData(radioSnapshot(await result.json()));
  };
  return (
    <MantineProvider theme={theme} defaultColorScheme="dark">
      <injectedStore.Provider>
        <SidebarCommandsProvider
          value={{
            open: open,
            newChat: () => (window.ordinary = true),
            newSharedChat: (path) => setShow({ path }),
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
            mobile={false}
            close={() => {}}
            rows={[]}
            markingRead={new Set()}
          />
        </SidebarCommandsProvider>
      </injectedStore.Provider>
      <Modal
        opened={!!show}
        title="New shared chat"
        onClose={() => setShow(null)}
        size="md"
      >
        {show && (
          <SharedChatCreate
            source={{
              startup: { creationScope: data.stateDir },
              state: {
                projects: new Map(
                  data.runtime.projects.map((project) => [project.id, project]),
                ),
                agents: new Map(
                  data.threads.map((agent) => [
                    agent.id,
                    {
                      ...agentSummaryFixture(agent.id),
                      ...agent,
                      inFlight: agent.inFlight === true,
                      projectServerId: agent.projectServerId ?? null,
                    },
                  ]),
                ),
                rooms: new Map(
                  data.runtime.rooms.map((room) => [room.id, room]),
                ),
              },
            }}
            accounts={accounts}
            initialPath={show.path}
            refresh={refresh}
            created={(id) => {
              open(id);
              setShow(null);
            }}
          />
        )}
      </Modal>
    </MantineProvider>
  );
}
createRoot(rootElement).render(<Fixture />);
