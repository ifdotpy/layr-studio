import { WorkspaceMode } from "../../generated/api";
import { useState } from "react";
import { Button } from "@mantine/core";
import { errorText } from "../../api";
import type { Agent } from "../../types";
import { useServerFileDialogs } from "../../servers/ServerServicesContext";
import "./retained-worktrees.css";
import { observer } from "mobx-react-lite";

import type { RetainedWorktreesData } from "../../generated/observableResources.ts";

type RetainedItem = RetainedWorktreesData[number];

/** The folder that cleanup kept for one worker, with its reason and path. */
export const RetainedWorktree = observer(function RetainedWorktree({
  agent,
  remoteWorker = "remoteWorker" in agent ? agent.remoteWorker : undefined,
}: {
  agent: Agent | RetainedItem;
  remoteWorker?: Agent["remoteWorker"];
}) {
  const { canRevealPath, revealPath } = useServerFileDialogs();
  const [error, setError] = useState("");
  const cleanup =
    "cleanup" in agent ? agent.cleanup : agent.workspaceCleanupIntent;
  const canOpen = !remoteWorker && canRevealPath();
  if (!cleanup || !["not_sent", "unknown"].includes(cleanup.phase)) return null;
  return (
    <section className="retained-worktree" aria-label="Retained worktree">
      <p>
        <strong>
          {cleanup.phase === "unknown"
            ? "Cleanup outcome unconfirmed"
            : "cleanup" in agent ||
                agent.worktree ||
                agent.workspaceMode === WorkspaceMode.worktree
              ? "Worktree retained"
              : "Workspace retained"}
        </strong>
      </p>
      <p>
        {cleanup.reason ||
          "Cleanup did not confirm removal. Inspect the folder before another cleanup."}
      </p>
      {cleanup.path && <p className="retained-worktree-path">{cleanup.path}</p>}
      <Button
        size="compact-sm"
        disabled={!cleanup.path || !canOpen}
        onClick={async () => {
          setError("");
          try {
            if (cleanup.path) await revealPath(cleanup.path);
          } catch (failure) {
            setError(errorText(failure));
          }
        }}
      >
        Open folder
      </Button>
      {!canOpen && <p>Open this path on the server that owns the worktree.</p>}
      {error && <p role="alert">{error}</p>}
    </section>
  );
});

export function TeamRetainedWorktrees({
  items,
}: {
  items: readonly RetainedItem[];
}) {
  return items.map((item) => <RetainedRow key={item.id} item={item} />);
}

const RetainedRow = observer(function RetainedRow({
  item,
}: {
  item: RetainedItem;
}) {
  return (
    <div>
      <p>{item.name || item.id}</p>
      <RetainedWorktree agent={item} />
    </div>
  );
});
