import { mountConversationFixture } from "../../ConversationView/ConversationView.fixture.tsx";
import { autorun, runInAction } from "mobx";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import * as defaultApi from "../../../../servers/net/api.ts";
import type { Message } from "../../../../types.ts";
import { ServerStoreProvider } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { requestFixture } from "./requestFixtures.ts";
import { agentSummaryFixture } from "../../../../servers/agentSummaryFixture.ts";
import { approval, question } from "../../../../approvalFixtures.ts";
import Conversation from "../../ObservedConversation/ObservedConversation.tsx";
import { ServerAreasContextProvider as ConversationProvider } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { conversationActionsFixture } from "../../../shell/integration-support/observableFixture.tsx";
import { requestTestEnvironment } from "../Requests/requestTestFixture.ts";

const h = vi.hoisted(() => ({
  items: [{ id: "lead:item", role: "assistant", text: "" }] as Message[],
  empty: [],
  receipts: new Map(),
  liveText: new Map(),
  hidden: () => false,
  withdrawalPending: false,
  withdrawalReads: 0,
  get: vi.fn(async () => ({})),
  post: vi.fn(async () => ({})),
  storage: new Storage(),
}));
// Keep the actual Conversation, TurnHistory, request-card mapping and generated store.
// Unrelated delivery/composer services stay inert in this question projection witness.
vi.mock(
  "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerApi: () => ({ ...defaultApi, get: h.get, post: h.post }),
    useServerServices: () => null,
    useServerStorage: () => h.storage,
    useServerOutbox: () => ({}),
    useServerUploads: () => ({}),
  }),
);
vi.mock("../../../agents/support/state.ts", async (original) => ({
  ...(await original<typeof import("../../../agents/support/state.ts")>()),
  useWorkspace: () => ({ stateDir: "questions" }),
}));
vi.mock("../../../../usePendingCommands.ts", () => ({
  usePendingCommands: () => h.empty,
}));
vi.mock("../../../../conversation/liveText.ts", async (original) => ({
  ...(await original<typeof import("../../../../conversation/liveText.ts")>()),
  useLiveText: () => h.liveText,
}));
vi.mock("../../support/usePromptRecall.ts", () => ({
  usePromptRecall: () => ({}),
}));
vi.mock("../../support/useChatReadState.ts", () => ({
  useVisibleChatResult() {},
}));
vi.mock("../../../agents/support/WorkerModelPicker.tsx", () => ({
  useWorkerModels: () => ({ models: h.empty }),
}));
vi.mock("../../../../hooks/conversationData", () => ({
  useChatWait: () => undefined,
}));
vi.mock("../../Usage/Usage.tsx", async (original) => ({
  ...(await original<typeof import("../../Usage/Usage.tsx")>()),
  default: () => null,
}));
vi.mock("../../Dictation/Dictation.tsx", () => ({ Dictation: () => null }));
vi.mock("../../RealtimeVoice/RealtimeVoice.tsx", async (original) => ({
  ...(await original<typeof import("../../RealtimeVoice/RealtimeVoice.tsx")>()),
  default: () => null,
}));
vi.mock("../../prompt-composer/PromptInput/PromptInput.tsx", () => ({
  default: () => null,
}));
vi.mock(
  "../../transcript/SelectionQuote/SelectionQuote.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../transcript/SelectionQuote/SelectionQuote.tsx")
    >()),
    default: () => null,
  }),
);

requestTestEnvironment();
beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  delete window.__studioPromptComposerRenderProbe;
  h.withdrawalPending = false;
  h.withdrawalReads = 0;
  vi.useRealTimers();
});

it.each(["add", "remove"] as const)(
  "the real Conversation and inline mapping ignore outside request membership (%s)",
  async (operation) => {
    const request = approval({
      id: "question",
      agent: "lead",
      request: {
        kind: "agent_question",
        questions: [question("choice", "Continue?")],
      },
      params: { itemId: "item" },
    });
    const outside = { ...request, id: "outside-question", agent: "outside" };
    const f = requestFixture(
      operation === "add" ? [request] : [request, outside],
      [
        agentSummaryFixture("lead", {
          name: "Lead",
          rootId: "lead",
          isLead: true,
        }),
        agentSummaryFixture("outside", { rootId: "outside", isLead: true }),
      ],
      [{ id: "lead", canSend: true }],
    );
    const actions = conversationActionsFixture(() => {}, f.store);
    actions.navigation.selectedId = "lead";
    mountConversationFixture(actions);
    actions.selected.history.items = h.items;
    const root = createRoot(
      document.body.appendChild(document.createElement("div")),
    );
    let stopPartition = () => {};
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
                {
                  value: actions,
                },
                createElement(Conversation),
              ),
            ),
          ),
        ),
      );
      await act(async () => vi.advanceTimersByTimeAsync(100));
      const card = document.querySelector('[data-request="question"]');
      expect(card).not.toBeNull();
      expect(card?.getAttribute("data-message")).toBe("lead:item");
      expect(
        document.querySelectorAll('[data-request="question"]'),
      ).toHaveLength(1);
      let renders = 0;
      // The partition now belongs to the chat store; the layout stays asleep.
      stopPartition = autorun(() => {
        void actions.selected.conversation?.inlineRequests;
        renders++;
      });
      renders = 0;
      const publish = async (requests: (typeof request)[]) =>
        act(async () => {
          f.live.apply(
            requests.map((value) => ({
              collection: "request",
              id: value.id,
              value,
            })),
          );
          await vi.advanceTimersByTimeAsync(0);
        });
      await publish(operation === "add" ? [request, outside] : [request]);
      expect(renders, `outside request ${operation}`).toBe(0);
      expect(document.querySelector('[data-request="question"]')).toBe(card);
      renders = 0;
      await publish(operation === "add" ? [request] : [request, outside]);
      expect(
        renders,
        `outside request ${operation === "add" ? "removed" : "added"}`,
      ).toBe(0);
      expect(document.querySelector('[data-request="question"]')).toBe(card);
      await publish([
        {
          ...request,
          deferral: { active: true, at: 2, by: { kind: "owner" } },
        },
      ]);
      expect(
        renders,
        "this chat's deferral changes inline placement",
      ).toBeGreaterThan(0);
      expect(
        document.querySelector('[data-message="lead:item"][data-request]'),
      ).toBeNull();
      expect(
        document.querySelector('.request-deferred [data-request="question"]'),
      ).not.toBeNull();
    } finally {
      stopPartition();
      await act(async () => root.unmount());
      f.store.dispose();
    }
  },
);

it("a pending withdrawal causes no repeated partition invalidation across renders", async () => {
  const f = requestFixture(
    [],
    [agentSummaryFixture("lead", { rootId: "lead", isLead: true })],
    [{ id: "lead", canSend: true }],
  );
  const root = createRoot(
    document.body.appendChild(document.createElement("div")),
  );
  const actions = conversationActionsFixture(() => {}, f.store);
  actions.navigation.selectedId = "lead";
  mountConversationFixture(actions);
  const render = (sending: boolean) =>
    act(async () => {
      runInAction(() => {
        actions.delivery.sendingLock = sending ? Symbol() : null;
      });
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            ServerStoreProvider,
            { value: f.store },
            createElement(
              ConversationProvider,
              {
                value: actions,
              },
              createElement(Conversation),
            ),
          ),
        ),
      );
      await vi.advanceTimersByTimeAsync(0);
    });
  try {
    await render(false);
    const queue = actions.selected.conversation?.context.messageQueue;
    h.withdrawalPending = true;
    for (const sending of [true, false, true]) {
      const before = h.withdrawalReads;
      await render(sending);
      expect(
        h.withdrawalReads - before,
        "composer state changes keep the queue hook idle without re-invalidating its partition",
      ).toBe(0);
      expect(actions.selected.conversation?.context.messageQueue).toBe(queue);
      expect(document.querySelector("#send-state")?.textContent).toBe(
        sending ? "Sending…" : "",
      );
    }
  } finally {
    await act(async () => root.unmount());
    f.store.dispose();
  }
});
