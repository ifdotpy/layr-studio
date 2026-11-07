import { WorkspaceMode } from "../../generated/api";
import { useEffect, useState } from "react";
import { Button } from "@mantine/core";
import { errorText, type GetResult } from "../../api";
import type { Agent } from "../../types";
import {
  useServerFileDialogs,
  useServerSync,
} from "../../servers/ServerServicesContext";
import "./retained-worktrees.css";
import { observer } from "mobx-react-lite";

type RetainedItem =
  GetResult<"/api/retained-worktrees">["retainedWorktrees"][number];

/** The folder that cleanup kept for one worker, with its reason and path. */
export const RetainedWorktree = observer(function RetainedWorktree({
  agent,
  remoteWorker = agent.remoteWorker,
}: {
  agent: Agent;
  remoteWorker?: Agent["remoteWorker"];
}) {
  const { canRevealPath, revealPath } = useServerFileDialogs();
  const [error, setError] = useState("");
  const cleanup = agent.workspaceCleanupIntent;
  const canOpen = !remoteWorker && canRevealPath();
  if (!cleanup || !["not_sent", "unknown"].includes(cleanup.phase)) return null;
  return (
    <section className="retained-worktree" aria-label="Retained worktree">
      <p>
        <strong>
          {cleanup.phase === "unknown"
            ? "Cleanup outcome unconfirmed"
            : agent.worktree || agent.workspaceMode === WorkspaceMode.worktree
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

/**
 * The retained folders of deleted workers of this team. Cleanup can end after a worker left
 * the live team list; their source-published entities survive until cleanup settles.
 */
export function useRetainedWorktrees(
  leadId: string | undefined,
  members: Agent[],
): RetainedItem[] {
  const { subscribeStateProjection } = useServerSync();
  const [items, setItems] = useState<RetainedItem[]>([]);
  useEffect(() => {
    setItems([]);
    if (!leadId) return;
    let source: unknown;
    return subscribeStateProjection(
      (snapshot) => {
        if (!snapshot || source === snapshot.runtime.retainedWorktrees) return;
        source = snapshot.runtime.retainedWorktrees;
        setItems(
          (Array.isArray(snapshot.runtime.retainedWorktrees)
            ? snapshot.runtime.retainedWorktrees
            : []
          ).filter((item) => item.rootId === leadId),
        );
      },
      () => {},
    );
  }, [leadId, subscribeStateProjection]);
  return items.filter(
    (item) => !members.some((member) => member.id === item.id),
  );
}

export function TeamRetainedWorktrees({ items }: { items: RetainedItem[] }) {
  return items.map((item) => (
    <div key={item.id}>
      <p>{item.name || item.id}</p>
      <RetainedWorktree
        agent={{
          id: item.id,
          name: item.name,
          worktree: true,
          workspaceCleanupIntent: item.cleanup,
        }}
      />
    </div>
  ));
}
