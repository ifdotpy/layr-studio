import { useTeam } from "../TeamPanelContext.tsx";
import { observer } from "mobx-react-lite";
import { providerLabel } from "../../../generated/providerKind.ts";

import { ProviderMark } from "../../AccountTiles.tsx";
import { ChatTitle } from "../../chat-row/ChatTitle.tsx";
import { TeamRoleIcon, TeamStateIcon } from "../TeamFacts.tsx";
import ChatStatus from "../ChatStatus.tsx";
import { AgentElapsed } from "../AgentProgress.tsx";
import { useAgentDetail } from "../state.ts";
import type { WorkerProps } from "../WorkerCard.tsx";

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

const WorkerProvider = observer(function WorkerProvider({
  agent,
}: WorkerProps) {
  const provider = agent.provider || "";
  return (
    <span
      className="worker-provider"
      role="img"
      aria-label={providerLabel(provider)}
      title={providerLabel(provider)}
    >
      <ProviderMark provider={provider} />
    </span>
  );
});

const WorkerElapsed = observer(function WorkerElapsed({ agent }: WorkerProps) {
  const detail = useAgentDetail(agent);
  return <AgentElapsed agent={agent} detail={detail} />;
});

export function WorkerHeading({ agent }: WorkerProps) {
  return (
    <span className="worker-name-line">
      <WorkerIndicator agent={agent} />
      <WorkerProvider agent={agent} />
      <ChatTitle agent={agent} />
      <TeamRoleIcon agent={agent} lead={false} />
      <WorkerElapsed agent={agent} />
    </span>
  );
}
