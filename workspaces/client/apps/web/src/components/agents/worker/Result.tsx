import { observer } from "mobx-react-lite";
import { Button } from "@mantine/core";
import { useAgentDetail } from "../state.ts";
import { useTeam } from "../TeamPanelContext.tsx";
import type { WorkerProps } from "../WorkerCard.tsx";

export const WorkerResult = observer(function WorkerResult({
  agent,
}: WorkerProps) {
  const detail = useAgentDetail(agent);
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
