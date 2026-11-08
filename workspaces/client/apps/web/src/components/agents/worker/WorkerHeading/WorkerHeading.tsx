import { WorkerIndicator } from "../WorkerIndicator/WorkerIndicator.tsx";
import { useServerStore } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { resourceData } from "../../../../store/runtime.ts";
import { agentId as observableAgentId } from "../../../../generated/observableResources.ts";
import { observer } from "mobx-react-lite";
import { providerLabel } from "../../../../generated/providerKind.ts";
import { ProviderMark } from "../../../accounts/ProviderMark/ProviderMark.tsx";
import { ChatTitle } from "../../../sidebar/chat-row/ChatTitle/ChatTitle.tsx";
import { TeamRoleIcon } from "../../TeamRoleIcon/TeamRoleIcon.tsx";
import { AgentElapsed } from "../../AgentProgress/AgentProgress.tsx";
import type { WorkerProps } from "../../WorkerCard/WorkerCard.tsx";

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
  const store = useServerStore();
  const detail = resourceData(
    store.agentDetail(observableAgentId(agent.id)).current,
  );
  return <AgentElapsed agent={agent} detail={detail} />;
});
