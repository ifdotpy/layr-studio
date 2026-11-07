import { useServerStore } from "../../../servers/ServerServicesContext.tsx";
import { resourceData } from "../../../store/runtime.ts";
import { agentId as observableAgentId } from "../../../generated/observableResources.ts";
import { observer } from "mobx-react-lite";
import { Button } from "@mantine/core";

import { useTeam } from "../TeamPanelContext.tsx";
import type { WorkerProps } from "../WorkerCard.tsx";

export const WorkerResult = observer(function WorkerResult({
  agent,
}: WorkerProps) {
  const store = useServerStore();
  const detail = resourceData(
    store.agentDetail(observableAgentId(agent.id)).current,
  );
  const { previewResult } = useTeam();
  const path = detail?.overview?.resultFile;
  return path ? (
    <Button
      component="a"
      href={path}
      size="compact-xs"
      variant="subtle"
      onClick={(event) => {
        event.preventDefault();
        previewResult({ agent: agent.id, path });
      }}
    >
      Open submitted result
    </Button>
  ) : null;
});
