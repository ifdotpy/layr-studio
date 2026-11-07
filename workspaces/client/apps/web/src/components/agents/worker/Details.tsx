import { useServerStore } from "../../../servers/ServerServicesContext.tsx";
import { resourceData } from "../../../store/runtime.ts";
import { agentId as observableAgentId } from "../../../generated/observableResources.ts";
import { observer } from "mobx-react-lite";
import { CircleAlert, Terminal } from "lucide-react";
import Disclosure from "../../ui/Disclosure.tsx";
import { AgentProgress } from "../AgentProgress.tsx";

import { useTeam } from "../TeamPanelContext.tsx";
import { workerError } from "../workerPresentation.ts";
import WorkerExcerpt from "../WorkerExcerpt.tsx";
import type { WorkerProps } from "../WorkerCard.tsx";

export const WorkerError = observer(function WorkerError({
  agent,
}: WorkerProps) {
  const store = useServerStore();
  const detail = resourceData(
    store.agentDetail(observableAgentId(agent.id)).current,
  );
  const { stopReason, errorView } = workerError(agent, detail);
  return detail?.error ? (
    <Disclosure
      className={stopReason ? "worker-stop-details" : "worker-error-details"}
      ariaLabel={stopReason ? "Stop details" : "Error details"}
      summary={
        <>
          <CircleAlert aria-hidden="true" />
          <span className="sr-only">
            {stopReason ? "Stop details" : "Error details"}
          </span>
        </>
      }
    >
      <pre>{errorView.details || errorView.message}</pre>
    </Disclosure>
  ) : null;
});

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
