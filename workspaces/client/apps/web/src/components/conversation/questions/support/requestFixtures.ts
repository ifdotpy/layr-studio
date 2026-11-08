import {
  createStateMirror,
  createRequestsMirror,
  createQuestionHistoryMirror,
  createAgentDetailMirror,
} from "../../../../generated/observableResources.ts";
import type { components } from "../../../../generated/api.ts";
import { ServerStoreProvider } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { createElement, type ReactNode } from "react";
import type { RequestModel } from "../RequestArea/RequestArea.tsx";
import { createServerStore } from "../../../../store/server.ts";
import { createSubscriptions } from "../../../../sync/subscriptions/module.ts";

/** Static generated resources for isolated question stories and tests. */
export function requestFixture(
  requests: readonly RequestModel[],
  agents: components["schemas"]["AgentSummaryEntityDto"][] = [],
  details: components["schemas"]["AgentDetailEntityDto"][] = [],
) {
  const state = createStateMirror(),
    live = createRequestsMirror(),
    history = createQuestionHistoryMirror();
  const owners = new Map(
    details.map((detail) => [detail.id, createAgentDetailMirror(detail)]),
  );
  live.apply(
    requests.map((value) => ({ collection: "request", id: value.id, value })),
  );
  state.apply(
    agents.map((value) => ({
      collection: "agentSummary",
      id: value.id,
      value,
    })),
  );
  const subscriptions = createSubscriptions({
    watchResourceChanges: () => () => {},
    watchResourceConnection: () => () => {},
  });
  const resources = createServerStore(subscriptions);
  const store: ReturnType<typeof createServerStore> = {
    ...resources,
    state: () => ({ current: { status: "live", data: state.current } }),
    requests: () => ({ current: { status: "live", data: live.current } }),
    questionHistory: () => ({
      current: { status: "live", data: history.current },
    }),
    agentDetail: (id) => {
      const detail = owners.get(id);
      return {
        current: detail
          ? { status: "live", data: detail.current }
          : { status: "loading" },
      };
    },
    dispose() {
      resources.dispose();
      subscriptions.dispose();
    },
  };
  return { store, state, live, history };
}
export function RequestFixture({
  requests,
  children,
  agents,
  details,
}: {
  requests: readonly RequestModel[];
  children?: ReactNode;
  agents?: Parameters<typeof requestFixture>[1];
  details?: Parameters<typeof requestFixture>[2];
}) {
  const { store } = requestFixture(requests, agents, details);
  return createElement(ServerStoreProvider, { value: store }, children);
}
