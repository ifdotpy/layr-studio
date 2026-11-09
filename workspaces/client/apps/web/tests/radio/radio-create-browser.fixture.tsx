import "../../src/rendererStyles.ts";
import { defaultSyncClient, syncDatabase } from "../../src/sync/client.ts";
import { when } from "mobx";
await syncDatabase();
// The isolated page follows App startup: apply its first SSE baseline before mounting forms.
const releaseInitialState = defaultSyncClient.retainStateDelivery(() => {});
await when(() => defaultSyncClient.projectionMetadata.ready);
window.addEventListener("pagehide", releaseInitialState, { once: true });
import type { ComponentSnapshot as Snapshot } from "../../src/components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { SidebarCommandsProvider } from "../../src/components/sidebar/support/commands.ts";
import { resourceData } from "../../src/store/runtime.ts";
import { observer } from "mobx-react-lite";
import { useServerStore } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { ServerViewStoreProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
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
    eligibility: { kind: "available" as const },
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
const Fixture = observer(function Fixture() {
  const store = useServerStore();
  const collections = resourceData(store.state().current);
  const data = initial;
  const [show, setShow] = useState<{ path?: string | undefined } | null>({
    path: "/p",
  });
  const [opened, open] = useState<string | null>(null);
  window.radioCreateSnapshot = data;
  window.radioOpened = opened;
  window.reopen = () => setShow({ path: "/p" });

  return (
    <MantineProvider theme={theme} defaultColorScheme="dark">
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
          markUnread: () => {},
        }}
      >
        <Sidebar
          data={data}
          opened={opened}
          creating={false}
          mobile={false}
          close={() => {}}
          {...(collections ? { collections } : {})}
          rows={[]}
          markingRead={new Set()}
        />
      </SidebarCommandsProvider>
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
              state: collections,
            }}
            accounts={accounts}
            initialPath={show.path}
            created={(id) => {
              open(id);
              setShow(null);
            }}
          />
        )}
      </Modal>
    </MantineProvider>
  );
});
createRoot(rootElement).render(
  <ServerViewStoreProvider>
    <Fixture />
  </ServerViewStoreProvider>,
);
