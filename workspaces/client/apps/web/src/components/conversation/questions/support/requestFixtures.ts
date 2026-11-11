import {
  createStateMirror,
  createRequestsMirror,
  createQuestionHistoryMirror,
  createAgentDetailMirror,
} from "../../../../generated/observableResources.ts";
import type { components } from "../../../../generated/api.ts";
import { ServerStoreProvider } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { createElement, type ReactNode } from "react";
import {
  RequestArea,
  RequestAreaOwner,
  type RequestCommands,
  type RequestModel,
} from "../RequestArea/RequestArea.tsx";
import { createServerStore } from "../../../../store/server.ts";
import * as api from "../../../../servers/net/api.ts";
import { serverLocalStorage } from "../../../../servers/storage.ts";
import { fixturePostResult } from "../../../../../tests/misc-fixture.ts";
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
type FixtureProps = RequestCommands & {
  requests: readonly RequestModel[];
  children?: ReactNode;
  agents?: Parameters<typeof requestFixture>[1];
  details?: Parameters<typeof requestFixture>[2];
};
/** Created, started, updated and disposed by the story or test harness. */
export function createRequestFixture({
  requests,
  agents,
  details,
  ...commands
}: Omit<FixtureProps, "children">) {
  const fixture = requestFixture(requests, agents, details);
  const owner = new RequestAreaOwner(
    {
      api: () => api,
      store: fixture.store,
      storage: serverLocalStorage,
      server: "local",
    },
    () => commands,
  );
  return {
    ...fixture,
    owner,
    start: owner.start,
    updateRequests(requests: readonly RequestModel[]) {
      fixture.live.apply(
        requests.map((value) => ({
          collection: "request",
          id: value.id,
          value,
        })),
      );
    },
    dispose() {
      owner.dispose();
      fixture.store.dispose();
    },
  };
}
export function RequestFixture({
  owner,
  children,
}: {
  owner: ReturnType<typeof createRequestFixture>;
  children?: ReactNode;
}) {
  return createElement(
    ServerStoreProvider,
    { value: owner.store },
    createElement(RequestArea, { owner: owner.owner }, children),
  );
}

/** Route response boundary for tests that record request commands. */
export function requestApiFixture(controls: {
  post(...args: unknown[]): Promise<unknown>;
  answer(...args: unknown[]): Promise<unknown>;
}): import("../../../../api.ts").ServerApi {
  return {
    ...api,
    post: async <Path extends import("../../../../apiTypes.ts").ApiPostPath>(
      path: Path,
      body: import("../../../../servers/net/api.ts").PostInput<Path>,
      options?: import("../../../../api.ts").PostOptions,
    ) => fixturePostResult<Path>(await controls.post(path, body, options)),
    command: {
      ...api.command,
      answer: async (...args) =>
        fixturePostResult<"/api/commands">(await controls.answer(...args)),
    },
  };
}
