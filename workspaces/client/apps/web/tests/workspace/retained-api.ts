import { createServerStore } from "../../src/store/server.ts";
import { createSubscriptions } from "../../src/sync/subscriptions/module.ts";
import { agentSummaryFixture } from "../../src/servers/agentSummaryFixture.ts";

declare global {
  interface Window {
    refreshRetained: () => Promise<void>;
    openedFolder: string;
  }
}
const subscriptions = createSubscriptions({
  watchResourceChanges: () => () => {},
  watchResourceConnection: (accept) => {
    accept("live", "open", {});
    return () => {};
  },
});
const store = createServerStore(subscriptions);
subscriptions.publish({ kind: "state" }, [
  {
    collection: "agentSummary",
    id: "worker",
    value: agentSummaryFixture("worker", { isLead: false, rootId: "lead" }),
  },
]);
window.refreshRetained = async () => {
  subscriptions.publish({ kind: "retained-worktrees", agentId: "lead" }, [
    {
      id: "deleted-worker",
      rootId: "lead",
      name: "Deleted worker",
      cleanup: {
        phase: "unknown",
        reason: null,
        path: "/Users/example/Projects/studio/.worktrees/codex-agents/deleted-worker",
      },
    },
  ]);
};
export const useServerStore = () => store;
export const useServerFileDialogs = () => ({
  canRevealPath: () => true,
  revealPath: async (path: string) => {
    window.openedFolder = path;
  },
});
