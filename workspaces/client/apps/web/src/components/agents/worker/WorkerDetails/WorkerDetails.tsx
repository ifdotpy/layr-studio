import { WorkerError } from "../WorkerError/WorkerError.tsx";
import { useServerStore } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { resourceData } from "../../../../store/runtime.ts";
import { agentId as observableAgentId } from "../../../../generated/observableResources.ts";
import { observer } from "mobx-react-lite";
import { Terminal } from "lucide-react";
import Disclosure from "../../../ui/Disclosure/Disclosure.tsx";
import { AgentProgress } from "../../AgentProgress/AgentProgress.tsx";
import { useTeam } from "../../support/TeamPanelContext.tsx";
import WorkerExcerpt from "../../WorkerExcerpt/WorkerExcerpt.tsx";
import type { WorkerProps } from "../../WorkerCard/WorkerCard.tsx";

export function WorkerDetails({ agent }: WorkerProps) {
  return (
    <div className="worker-disclosures">
      <WorkerError agent={agent} />
      <WorkerReport agent={agent} />
      <WorkerProgress agent={agent} />
      <WorkerStall agent={agent} />
    </div>
  );
}

const WorkerReport = observer(function WorkerReport({ agent }: WorkerProps) {
  const store = useServerStore();
  const detail = resourceData(
    store.agentDetail(observableAgentId(agent.id)).current,
  );
  const { open } = useTeam();
  const overview = detail?.overview;
  return overview?.result ? (
    <WorkerExcerpt
      agentId={agent.id}
      label="Last report"
      text={overview.result}
      truncated={overview.resultTruncated ?? undefined}
      open={() => open(agent.id)}
    />
  ) : null;
});

const WorkerProgress = observer(function WorkerProgress({
  agent,
}: WorkerProps) {
  const store = useServerStore();
  const detail = resourceData(
    store.agentDetail(observableAgentId(agent.id)).current,
  );
  return detail?.commandProgress?.length ? (
    <Disclosure
      className="worker-command-details"
      ariaLabel="Command progress"
      summary={
        <>
          <Terminal aria-hidden="true" />
          <span className="sr-only">Command progress</span>
        </>
      }
    >
      <AgentProgress agent={detail} />
    </Disclosure>
  ) : null;
});

const WorkerStall = observer(function WorkerStall({ agent }: WorkerProps) {
  return agent.stall ? (
    <span className="agent-stall" title="Waiting for lead: task not submitted">
      Waiting for lead
    </span>
  ) : null;
});
