import { observer } from "mobx-react-lite";
import { isDeferred } from "../../../../approvals.ts";
import type { RequestScope } from "../../support/requestView.ts";
import {
  requestRenderProbe,
  useRequestOwner,
} from "../RequestArea/RequestArea.tsx";
import { RequestCard } from "../RequestCard/RequestCard.tsx";

export { openRequestAnswer } from "../RequestCard/RequestCard.tsx";

const RequestGroup = observer(function RequestGroup({
  requests,
  showDates,
}: {
  requests: readonly import("../RequestArea/RequestArea.tsx").RequestModel[];
  showDates?: boolean | undefined;
}) {
  const deferred = requests.filter(isDeferred);
  const cards = (items: typeof requests) =>
    items.map((request) => (
      <RequestCard key={request.id} request={request} showDates={showDates} />
    ));
  return (
    <>
      {cards(requests.filter((request) => !isDeferred(request)))}
      {deferred.length > 0 && (
        <details className="request-history request-deferred">
          <summary>
            Later <span>{deferred.length}</span>
          </summary>
          {cards(deferred)}
        </details>
      )}
    </>
  );
});
export default observer(function Requests({
  excludeIds,
  requestIds,
  showDates,
  ...scope
}: RequestScope & {
  showDates?: boolean | undefined;
  excludeIds?: ReadonlySet<string>;
  requestIds?: readonly string[];
}) {
  const view = useRequestOwner().list;
  const requests = (
    Object.keys(scope).length ? view.select(scope) : view.requests
  ).filter(
    (request) =>
      (!excludeIds || !excludeIds.has(request.id)) &&
      (!requestIds || requestIds.includes(request.id)),
  );
  const owner = Object.keys(scope).length
    ? scope.agentId
      ? view.agents?.get(scope.agentId)
      : undefined
    : view.owner;
  const mainAgentId = owner?.isLead === false ? owner.rootId : undefined;
  const mainRequests = mainAgentId
    ? requests.filter((request) => request.agent === mainAgentId)
    : [];
  requestRenderProbe("list");
  return (
    <div id="requests">
      {mainRequests.length > 0 && (
        <section aria-label="Main-agent requests">
          <h3 className="main-agent-requests-heading">Main-agent requests</h3>
          <RequestGroup requests={mainRequests} showDates={showDates} />
        </section>
      )}
      <RequestGroup
        requests={
          mainAgentId
            ? requests.filter((request) => request.agent !== mainAgentId)
            : requests
        }
        showDates={showDates}
      />
    </div>
  );
});
