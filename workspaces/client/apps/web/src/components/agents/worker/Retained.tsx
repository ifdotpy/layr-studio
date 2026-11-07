import { observer } from "mobx-react-lite";
import { RetainedWorktree } from "../RetainedWorktrees.tsx";
import { useAgentDetail } from "../state.ts";
import type { WorkerProps } from "../WorkerCard.tsx";

export const WorkerRetainedWorktree = observer(function WorkerRetainedWorktree({
  agent,
}: WorkerProps) {
  const detail = useAgentDetail(agent);
  return detail ? (
    <RetainedWorktree agent={detail} remoteWorker={agent.remoteWorker} />
  ) : null;
});
