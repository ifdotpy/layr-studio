import { useServerStore } from "../../../servers/ServerServicesContext.tsx";
import { resourceData } from "../../../store/runtime.ts";
import { agentId as observableAgentId } from "../../../generated/observableResources.ts";
import { observer } from "mobx-react-lite";
import { RetainedWorktree } from "../RetainedWorktrees.tsx";

import type { WorkerProps } from "../WorkerCard.tsx";

export const WorkerRetainedWorktree = observer(function WorkerRetainedWorktree({
  agent,
}: WorkerProps) {
  const store = useServerStore();
  const detail = resourceData(
    store.agentDetail(observableAgentId(agent.id)).current,
  );
  return detail ? (
    <RetainedWorktree agent={detail} remoteWorker={agent.remoteWorker} />
  ) : null;
});
