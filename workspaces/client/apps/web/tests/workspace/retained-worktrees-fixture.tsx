const fixtureServerStore = createServerStore(defaultSubscriptions);
window.addEventListener("pagehide", () => fixtureServerStore.dispose(), {
  once: true,
});
import { createServerStore } from "../../src/store/server.ts";
import { defaultSubscriptions } from "../../src/sync/subscriptions/events.ts";
import "../../src/rendererStyles.ts";
import { ServerStoreProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type { Agent } from "../../src/types.ts";

import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import { RetainedWorktree } from "../../src/components/agents/TeamRetainedWorktrees/TeamRetainedWorktrees.tsx";
import { TeamRetainedWorktrees } from "../../src/components/agents/TeamRetainedWorktrees/TeamRetainedWorktrees.tsx";
import { observer } from "mobx-react-lite";
import { useTeamArea } from "../../src/components/agents/support/TeamPanelContext.tsx";
const worker: Agent = {
  id: "worker",
  worktree: true,
  rootId: "lead",
  status: "completed",
  workspaceCleanupIntent: {
    phase: "not_sent",
    reason:
      "Submodule vendor/documentation has local changes. Cleanup kept the worktree.",
    path: "/Users/example/Projects/studio/.worktrees/codex-agents/worker/vendor/documentation",
  },
};
const Team = observer(function Team() {
  const area = useTeamArea("lead", "", "all");
  return (
    <>
      <TeamRetainedWorktrees items={area.retainedWorktrees()} />
      <RetainedWorktree agent={worker} />
    </>
  );
});
createRoot(document.getElementById("root")!).render(
  <MantineProvider>
    <ServerStoreProvider value={fixtureServerStore}>
      <main
        style={{
          maxWidth: 900,
          margin: "auto",
          padding: 12,
          display: "grid",
          gap: 12,
        }}
      >
        <h1>Team</h1>
        <Team />
      </main>
    </ServerStoreProvider>
  </MantineProvider>,
);
