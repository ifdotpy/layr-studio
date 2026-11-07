import type { Agent } from "../../src/types.ts";

import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import {
  RetainedWorktree,
  TeamRetainedWorktrees,
} from "../../src/components/agents/RetainedWorktrees.tsx";
import { observer } from "mobx-react-lite";
import { useTeamArea } from "../../src/components/agents/TeamPanelContext.tsx";
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
  </MantineProvider>,
);
