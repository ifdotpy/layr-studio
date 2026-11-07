import { useServerStore } from "../../../servers/ServerServicesContext.tsx";
import { resourceData } from "../../../store/runtime.ts";
import { agentId as observableAgentId } from "../../../generated/observableResources.ts";
import { observer } from "mobx-react-lite";
import { accountDisplayName } from "../../../accountName.ts";
import TokenRate from "../../TokenRate.tsx";
import { useWorkerModels } from "../WorkerModelPicker.tsx";
import { useTeam } from "../TeamPanelContext.tsx";

import { workerMeta } from "../workerPresentation.ts";
import type { WorkerProps } from "../WorkerCard.tsx";

const WorkerTokenRate = observer(function WorkerTokenRate({
  agent,
}: WorkerProps) {
  return <TokenRate agent={agent} variant="worker" />;
});

export const WorkerMetadata = observer(function WorkerMetadata({
  agent,
}: WorkerProps) {
  const store = useServerStore();
  const detail = resourceData(
    store.agentDetail(observableAgentId(agent.id)).current,
  );
  const { view } = useTeam();
  const accounts =
    resourceData(store.state().current)?.accountsSummary.get("current")
      ?.accounts ?? [];
  const catalog = useWorkerModels(
    view.lead?.accountKey || "default",
    true,
    true,
  );
  const { meta, modelName, effort, connected, account } = workerMeta(
    agent,
    detail,
    catalog.models,
    accounts,
  );
  return (
    <span className="worker-meta" title={meta}>
      <span className="worker-model-name">{modelName}</span>
      <span className="worker-effort">
        {effort.charAt(0).toUpperCase() + effort.slice(1)}
      </span>
      {connected.length > 1 && account && (
        <span className="worker-account">{accountDisplayName(account)}</span>
      )}
      <WorkerTokenRate agent={agent} />
    </span>
  );
});
