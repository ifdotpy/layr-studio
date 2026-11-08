import { useTeam } from "../../support/TeamPanelContext.tsx";
import { observer } from "mobx-react-lite";
import { TeamStateIcon } from "../../TeamStateIcon/TeamStateIcon.tsx";
import ChatStatus from "../../ChatStatus/ChatStatus.tsx";
import type { WorkerProps } from "../../WorkerCard/WorkerCard.tsx";

export const WorkerIndicator = observer(function WorkerIndicator({
  agent,
}: WorkerProps) {
  const { view } = useTeam();
  const indicator = view.indicator(agent);
  return (
    <span className="worker-indicator">
      <ChatStatus agent={agent} pendingRead={view.pendingRead(agent)} />
      {["none", "paused"].includes(indicator.kind) && (
        <TeamStateIcon kind={indicator.kind} />
      )}
    </span>
  );
});
