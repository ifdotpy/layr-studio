import { useServerStore } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { resourceData } from "../../../../store/runtime.ts";
import { agentId as observableAgentId } from "../../../../generated/observableResources.ts";
import { observer } from "mobx-react-lite";
import { CircleAlert } from "lucide-react";
import Disclosure from "../../../ui/Disclosure/Disclosure.tsx";
import { workerError } from "../../support/workerPresentation.ts";
import type { WorkerProps } from "../../WorkerCard/WorkerCard.tsx";

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
