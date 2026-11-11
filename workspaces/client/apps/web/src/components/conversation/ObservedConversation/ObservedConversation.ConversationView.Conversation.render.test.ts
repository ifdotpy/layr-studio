import { projectedLimits } from "../../../usage/accountLimits.fixture.ts";
import { serializeStateCollections } from "../../../generated/observableResources.ts";
import { AccountStatus, ProviderKind } from "../../../generated/api.ts";
import { action, observable } from "mobx";
// @vitest-environment happy-dom
import { act, createElement, memo, Profiler } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import * as defaultApi from "../../../servers/net/api.ts";
import type { Message } from "../../../types.ts";
import { ServerStoreProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { requestFixture } from "../questions/support/requestFixtures.ts";
import { agentSummaryFixture } from "../../../servers/agentSummaryFixture.ts";
import Conversation from "./ObservedConversation.tsx";
import { mountConversationFixture } from "../ConversationView/ConversationView.fixture.tsx";
import { approval, question } from "../../../approvalFixtures.ts";
import {
  RequestArea,
  useRequestOwner,
  RequestAreaOwner,
} from "../questions/RequestArea/RequestArea.tsx";
import { ConversationProvider } from "../support/navigation.ts";
import { conversationActionsFixture } from "../../shell/integration-support/observableFixture.tsx";
import { requestTestEnvironment } from "../questions/Requests/requestTestFixture.ts";

const h = vi.hoisted(() => ({
  commits: [] as string[],
  items: [{ id: "lead:item", role: "assistant", text: "" }] as Message[],
  empty: [],
  queue: { items: [] },
  liveText: new Map(),
  get: vi.fn(async () => ({})),
  post: vi.fn(async () => ({})),
  storage: new Storage(),
}));
// Exercise the actual Conversation/context owner with an isolated profiled consumer.
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerApi: () => ({ ...defaultApi, get: h.get, post: h.post }),
    useServerServices: () => null,
    useServerStorage: () => h.storage,
    useServerOutbox: () => ({}),
    useServerUploads: () => ({}),
  }),
);
vi.mock("../../agents/support/state.ts", async (original) => ({
  ...(await original<typeof import("../../agents/support/state.ts")>()),
  useWorkspace: () => ({ stateDir: "questions" }),
}));
vi.mock("../../usePendingCommands", () => ({
  usePendingCommands: () => h.empty,
}));
vi.mock("../../../conversation/liveText.ts", async (original) => ({
  ...(await original<typeof import("../../../conversation/liveText.ts")>()),
  useLiveText: () => h.liveText,
}));
vi.mock("../support/usePromptRecall.ts", () => ({
  usePromptRecall: () => ({}),
}));
vi.mock("../support/useChatReadState.ts", () => ({
  useVisibleChatResult() {},
}));
vi.mock("../../agents/support/WorkerModelPicker.tsx", () => ({
  useWorkerModels: () => ({ models: h.empty }),
}));
vi.mock("../../../hooks/conversationData", () => ({
  useChatWait: () => undefined,
}));
vi.mock("../Usage/Usage.tsx", async (original) => ({
  ...(await original<typeof import("../Usage/Usage.tsx")>()),
  default: () => null,
}));
vi.mock("../Dictation/Dictation.tsx", () => ({ Dictation: () => null }));
vi.mock("../RealtimeVoice/RealtimeVoice.tsx", async (original) => ({
  ...(await original<typeof import("../RealtimeVoice/RealtimeVoice.tsx")>()),
  default: () => null,
}));
vi.mock("../prompt-composer/PromptInput/PromptInput.tsx", () => ({
  default: () => null,
}));
vi.mock(
  "../transcript/SelectionQuote/SelectionQuote.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../transcript/SelectionQuote/SelectionQuote.tsx")
    >()),
    default: () => null,
  }),
);

requestTestEnvironment();
beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  delete window.__studioPromptComposerRenderProbe;
  vi.useRealTimers();
});

vi.mock("../transcript/TurnHistory/TurnHistory.tsx", async () => {
  const { useTranscriptActions } = await import("../support/navigation.ts");
  const Consumer = memo(function Consumer() {
    const { renderMessage } = useTranscriptActions();
    return createElement(Profiler, {
      id: "transcript-consumer",
      onRender: (_id, phase) => h.commits.push(phase),
      children: createElement(
        "span",
        null,
        "Transcript consumer",
        ...h.items.filter((item) => item.nativeNotice).map(renderMessage),
      ),
    });
  });
  return { default: () => createElement(Consumer) };
});

it.each(["scroll", "resize"] as const)(
  "%s leaves preserved TranscriptProvider consumers idle",
  async (event) => {
    const f = requestFixture(
      [
        approval({
          id: "question",
          agent: "lead",
          request: {
            kind: "agent_question",
            questions: [question("choice", "Continue?")],
          },
          params: { itemId: "item" },
        }),
      ],
      [agentSummaryFixture("lead", { rootId: "lead", isLead: true })],
      [{ id: "lead", canSend: true }],
    );
    const actions = conversationActionsFixture(() => {}, f.store);
    actions.navigation.selectedId = "lead";
    mountConversationFixture(actions);
    actions.selected.history.items = h.items;
    const host = document.body.appendChild(document.createElement("div"));
    const root = createRoot(host);
    try {
      await act(async () =>
        root.render(
          createElement(
            MantineProvider,
            null,
            createElement(
              ServerStoreProvider,
              { value: f.store },
              createElement(
                ConversationProvider,
                { value: actions },
                createElement(Conversation),
              ),
            ),
          ),
        ),
      );
      await act(async () => vi.advanceTimersByTimeAsync(100));
      expect(h.commits.length).toBeGreaterThan(0);
      h.commits = [];
      await act(async () => {
        if (event === "scroll") {
          host
            .querySelector("#messages")
            ?.dispatchEvent(new Event("scroll", { bubbles: true }));
          action(() => {
            actions.selected.history.before = "older-page";
          })();
        } else {
          actions.layout.recordResize({
            ...actions.layout.geometry,
            viewport: { x: 0, y: 0, width: 1440, height: 900 },
            host: { x: 0, y: 0, width: 1440, height: 900 },
          });
          window.dispatchEvent(new Event("resize"));
        }
        await vi.advanceTimersByTimeAsync(100);
      });
      console.log(
        event,
        "TranscriptProvider consumer commits",
        h.commits.length,
      );
      expect(h.commits).toHaveLength(0);
    } finally {
      await act(async () => root.unmount());
      host.remove();
      f.store.dispose();
      actions.scope.services.close();
    }
  },
);

it("RequestArea preserves its command context on an unchanged parent render", async () => {
  const commits: string[] = [];
  const Consumer = memo(function Consumer() {
    useRequestOwner();
    return createElement(Profiler, {
      id: "request-consumer",
      onRender: (_id, phase) => commits.push(phase),
      children: createElement("span", null, "Request"),
    });
  });
  const owner = new RequestAreaOwner(
    {
      api: () => defaultApi,
      store: requestFixture([]).store,
      storage: h.storage,
      server: "local",
    },
    () => ({ scope: "fixture", notify() {} }),
  );
  const commands = {
    owner,
    scope: "fixture",
    refresh: async () => {},
    notify: () => {},
  };
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  try {
    const render = () =>
      act(async () =>
        root.render(
          createElement(RequestArea, commands, createElement(Consumer)),
        ),
      );
    await render();
    expect(commits).toEqual(["mount"]);
    commits.length = 0;
    await render();
    console.log("RequestArea unchanged parent commits", commits.length);
    expect(commits).toHaveLength(0);
  } finally {
    await act(async () => root.unmount());
    host.remove();
  }
});

it("an account identity readback with the same plan leaves Conversation idle", async () => {
  const f = requestFixture(
    [],
    [agentSummaryFixture("lead", { rootId: "lead", isLead: true })],
    [{ id: "lead", canSend: true }],
  );
  const limits = projectedLimits("default", 100);
  limits.observation.data = {
    accountId: "native",
    rateLimits: { planType: "plus" },
  };
  const account = {
    id: "default",
    accountId: "native",
    label: "Account",
    provider: ProviderKind.codex,
    status: AccountStatus.ready,
    home: "",
    source: "fixture",
  };
  const apply = (accountId: string) =>
    f.state.apply([
      ...serializeStateCollections(f.state.current).filter(
        (row) =>
          row.collection !== "accounts" && row.collection !== "rateLimit",
      ),
      {
        collection: "accounts",
        id: "current",
        value: {
          accounts: [{ ...account, accountId }],
          archivedAccounts: [],
          defaultAccountKey: "default",
          defaultAccountGroups: [],
          supportsDelete: true,
          supportsDisconnect: true,
        },
      },
      {
        collection: "rateLimit",
        id: "default",
        value: {
          ...limits,
          observation: {
            ...limits.observation,
            data: { accountId, rateLimits: { planType: "plus" } },
          },
        },
      },
    ]);
  apply("native");
  const actions = conversationActionsFixture(() => {}, f.store);
  actions.navigation.selectedId = "lead";
  mountConversationFixture(actions);
  let renders = 0;
  window.__studioPromptComposerRenderProbe = (component) => {
    if (component === "conversation") renders++;
  };
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            ServerStoreProvider,
            { value: f.store },
            createElement(
              ConversationProvider,
              { value: actions },
              createElement(Conversation),
            ),
          ),
        ),
      ),
    );
    await act(async () => vi.advanceTimersByTimeAsync(100));
    renders = 0;
    h.get.mockClear();
    await act(async () => apply("native-updated"));
    console.log("same plan readback Conversation renders", renders);
    expect(renders).toBe(0);
    expect(h.get).not.toHaveBeenCalled();
  } finally {
    await act(async () => root.unmount());
    host.remove();
    f.store.dispose();
    actions.scope.services.close();
  }
});

it("updates an existing transcript notice when SSE changes its fields in place", async () => {
  const item = observable<Message>({
    id: "native-notice",
    role: "system",
    text: "",
    nativeNotice: "error",
    nativeError: "Initial failure",
    details: "Initial detail",
    nativeHookQuiet: false,
  });
  const previous = h.items;
  h.items = [item];
  const f = requestFixture(
    [],
    [agentSummaryFixture("lead", { rootId: "lead", isLead: true })],
    [{ id: "lead", canSend: true }],
  );
  const actions = conversationActionsFixture(() => {}, f.store);
  actions.navigation.selectedId = "lead";
  mountConversationFixture(actions);
  actions.selected.history.items = h.items;
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            ServerStoreProvider,
            { value: f.store },
            createElement(
              ConversationProvider,
              { value: actions },
              createElement(Conversation),
            ),
          ),
        ),
      ),
    );
    expect(host.textContent).toContain("Initial failure");
    expect(host.textContent).toContain("Initial detail");
    await act(async () =>
      action(() => {
        item.nativeError = "Updated failure";
        item.details = "Updated detail";
      })(),
    );
    expect(host.textContent).toContain("Updated failure");
    expect(host.textContent).toContain("Updated detail");
    expect(host.textContent).not.toContain("Initial failure");
    await act(async () =>
      action(() => {
        item.nativeHookQuiet = true;
      })(),
    );
    expect(host.querySelector('[data-message="native-notice"]')).toBeNull();
  } finally {
    await act(async () => root.unmount());
    host.remove();
    f.store.dispose();
    actions.scope.services.close();
    h.items = previous;
  }
});

it("renders transcript requests and opens the selected owner's answer editor", async () => {
  const f = requestFixture(
    [
      approval({
        id: "owner-question",
        agent: "lead",
        request: {
          kind: "agent_question",
          questions: [question("choice", "Owned answer?")],
        },
      }),
    ],
    [agentSummaryFixture("lead", { rootId: "lead", isLead: true })],
    [{ id: "lead", canSend: true }],
  );
  const actions = conversationActionsFixture(() => {}, f.store);
  actions.navigation.selectedId = "lead";
  mountConversationFixture(actions);
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            ServerStoreProvider,
            { value: f.store },
            createElement(
              ConversationProvider,
              { value: actions },
              createElement(Conversation),
            ),
          ),
        ),
      ),
    );
    const view = actions.selected.requestOwner.views.get("owner-question")!;
    const card = host.querySelector('[data-request="owner-question"]');
    expect(card).not.toBeNull();
    await act(async () =>
      card!.querySelector<HTMLButtonElement>("[data-answer]")!.click(),
    );
    expect(view.open).toBe(true);
    expect(
      host.querySelector('textarea[aria-label="Owned answer?"]'),
    ).not.toBeNull();
    await act(async () => view.close());
    expect(host.querySelector(".request-answer-form")).toBeNull();
  } finally {
    await act(async () => root.unmount());
    actions.selected.closeConversation();
    actions.scope.services.close();
    f.store.dispose();
  }
});
