import { observer } from "mobx-react-lite";
import { useContext } from "react";
import { accountDisplayName } from "../../../accountName.ts";
import TokenRate from "../../TokenRate.tsx";
import { TeamModelCatalog, TeamAccountCatalog } from "../WorkerOverview.tsx";
import { useAgentDetail } from "../state.ts";
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
  const detail = useAgentDetail(agent);
  const accounts = useContext(TeamAccountCatalog);
  const { meta, modelName, effort, connected, account } = workerMeta(
    agent,
    detail,
    useContext(TeamModelCatalog),
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
