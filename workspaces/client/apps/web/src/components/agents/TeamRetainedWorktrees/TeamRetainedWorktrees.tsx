import { WorkspaceMode } from "../../../generated/api.ts";
import { Owned } from "../../ui/Owned/Owned.tsx";
import { RetainedFolder } from "../support/RetainedFolder.ts";
import { Button } from "@mantine/core";
import type { Agent } from "../../../types.ts";
import {
  useServerFileDialogs,
  useServerOwner,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";

import { observer } from "mobx-react-lite";

import type { RetainedWorktreesData } from "../../../generated/observableResources.ts";

type RetainedItem = RetainedWorktreesData[number];

/** The folder that cleanup kept for one worker, with its reason and path. */
export const RetainedWorktree = observer(function RetainedWorktree({
  agent,
}: {
  agent: Agent | RetainedItem;
}) {
  const { canRevealPath, revealPath } = useServerFileDialogs();
  const owner = useServerOwner();
  const cleanup =
    "cleanup" in agent ? agent.cleanup : agent.workspaceCleanupIntent;
  const canOpen = canRevealPath();
  if (!cleanup || !["not_sent", "unknown"].includes(cleanup.phase)) return null;
  return (
    <Owned
      key={JSON.stringify([
        owner?.id,
        owner?.origin,
        owner?.credentialId,
        agent.id,
        cleanup.path,
      ])}
      create={() => new RetainedFolder()}
    >
      {(folder) => (
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
          {cleanup.path && (
            <p className="retained-worktree-path">{cleanup.path}</p>
          )}
          <Button
            size="compact-sm"
            disabled={!cleanup.path || !canOpen}
            onClick={() =>
              cleanup.path && void folder.open(cleanup.path, revealPath)
            }
          >
            Open folder
          </Button>
          {!canOpen && (
            <p>Open this path on the server that owns the worktree.</p>
          )}
          {folder.error && <p role="alert">{folder.error}</p>}
        </section>
      )}
    </Owned>
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
