import { useEffect } from "react";
import { autorun } from "mobx";
import { observer } from "mobx-react-lite";
import { useServerStorage } from "../../servers/ServerServicesContext";
import { isDeferred } from "../../approvals";
import { requestDraftsForStorage } from "./requestDrafts";
import { useRequestsView, type RequestScope } from "./requestView";
import {
  answerKey,
  requestRenderProbe,
  useRequestScope,
} from "./requestActions";
import { RequestCard } from "./RequestCard";
import "./request-questions.css";
export { RequestArea } from "./requestActions";
export { RequestCard, openRequestAnswer } from "./RequestCard";
export { AnswerForm } from "./AnswerForm";

const RequestGroup = observer(function RequestGroup({
  requests,
  showDates,
}: {
  requests: ReturnType<typeof useRequestsView>["requests"];
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
  const view = useRequestsView(scope),
    commands = useRequestScope();
  const drafts = requestDraftsForStorage(useServerStorage());
  useEffect(
    () =>
      autorun(() => {
        if (view.current.status !== "live" && view.current.status !== "stale")
          return;
        const active = new Set(
          view.allRequests.map((request) => answerKey(commands.scope, request)),
        );
        for (const key of drafts.keys())
          if (JSON.parse(key)[0] === commands.scope && !active.has(key))
            drafts.delete(key);
      }),
    [view, drafts, commands.scope],
  );
  const requests = view.requests.filter(
    (request) =>
      (!excludeIds || !excludeIds.has(request.id)) &&
      (!requestIds || requestIds.includes(request.id)),
  );
  const mainAgentId =
    view.owner?.isLead === false ? view.owner.rootId : undefined;
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
