import { serverLocalStorage } from "../../../../servers/storage.ts";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { observer } from "mobx-react-lite";
import { expect, it, vi } from "vitest";
import { createApiScope } from "../../../../servers/net/api.ts";
import { createMutationJournal } from "../../../../servers/mutationIdentity.ts";
import {
  API_SCHEMA_HASH,
  API_SCHEMA_HASH_HEADER,
} from "../../../../generated/apiSchema.ts";
import { createNet } from "../../../../servers/net/net.ts";
import { createServerStore } from "../../../../store/server.ts";
import { createSubscriptions } from "../../../../sync/subscriptions/module.ts";
import type { GetResult } from "../../../../api.ts";
import type { Agent, Message } from "../../../../types.ts";
import { useTurnErrors } from "./useTurnErrors.ts";

const hooks = vi.hoisted(() => ({
  net: vi.fn(),
  api: vi.fn(),
  store: vi.fn(),
}));
vi.mock(
  "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerNet: hooks.net,
    useServerApi: hooks.api,
    useServerStore: hooks.store,
  }),
);

it("shares observed analytics, enriches only its own thread, and stays idle after settling", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const subscriptions = createSubscriptions({
    watchResourceChanges: () => () => {},
    watchResourceConnection: () => () => {},
  });
  const store = createServerStore(subscriptions);
  const pending = Promise.withResolvers<GetResult<"/api/analytics">>();
  const fetch = vi.fn(() =>
    pending.promise.then(
      (data) =>
        new Response(JSON.stringify(data), {
          headers: {
            "Content-Type": "application/json",
            [API_SCHEMA_HASH_HEADER]: API_SCHEMA_HASH,
          },
        }),
    ),
  );
  const boundApi = createApiScope({
    origin: "https://server.test",
    fetch,
    localStorage: serverLocalStorage,
    sessionStorage,
    beginMutation: createMutationJournal("fixture", serverLocalStorage),
  });
  const net = createNet(boundApi, () => store);
  const analytics = vi.spyOn(net.reads, "analytics");
  hooks.net.mockReturnValue(net);
  hooks.api.mockReturnValue(boundApi);
  hooks.store.mockReturnValue(store);
  const host = document.createElement("div"),
    root = createRoot(host);
  const items: Message[] = [
    {
      id: "one",
      role: "assistant",
      text: "",
      turnId: "turn",
      turnStatus: "failed",
    },
  ];
  const agent: Agent = { id: "agent", threadId: "thread" };
  const View = observer(function View() {
    const result = useTurnErrors(items, agent);
    return createElement("span", null, JSON.stringify(result));
  });
  try {
    await act(async () =>
      root.render(
        createElement("div", null, createElement(View), createElement(View)),
      ),
    );
    expect(analytics).toHaveBeenCalled();
    expect(fetch).toHaveBeenCalledTimes(1);
    await act(async () =>
      pending.resolve({
        turns: [
          {
            agentId: "agent",
            threadId: "other",
            turnId: "turn",
            status: "failed",
            error: { message: "wrong reason" },
          },
          {
            agentId: "agent",
            threadId: "thread",
            turnId: "turn",
            status: "failed",
            error: { message: "saved reason" },
          },
        ],
      }),
    );
    expect(host.textContent).toContain("saved reason");
    expect(host.textContent).not.toContain("wrong reason");
    await act(async () =>
      root.render(createElement("div", null, createElement(View))),
    );
    expect(fetch).toHaveBeenCalledTimes(1);
  } finally {
    await act(async () => root.unmount());
    store.dispose();
    subscriptions.dispose();
    vi.unstubAllGlobals();
  }
});
