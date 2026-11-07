// @vitest-environment happy-dom
import { act, createElement, useRef } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import * as defaultApi from "../../api";
import type { Message } from "../../types";
import { ServerStoreProvider } from "../../servers/ServerServicesContext";
import { requestFixture } from "./requestFixtures";
import { agentSummaryFixture } from "../../servers/agentSummaryFixture";
import { approval, question } from "../../approvalFixtures";
import Conversation from "../Conversation";
import { ConversationProvider } from "../conversation/navigation.ts";
import { conversationActionsFixture } from "../observableFixture.tsx";
import { requestTestEnvironment } from "./requestTestFixture";

const h = vi.hoisted(() => ({
  items: [{ id: "lead:item", role: "assistant", text: "" }] as Message[],
  empty: [],
  receipts: new Map(),
  liveText: new Map(),
  hidden: () => false,
  get: vi.fn(async () => ({})),
  post: vi.fn(async () => ({})),
  storage: new Storage(),
}));
// Keep the actual Conversation, TurnHistory, request-card mapping and generated store.
// Unrelated delivery/composer services stay inert in this question projection witness.
vi.mock("../../servers/ServerServicesContext", async (original) => ({
  ...(await original<typeof import("../../servers/ServerServicesContext")>()),
  useServerApi: () => ({ ...defaultApi, get: h.get, post: h.post }),
  useServerServices: () => null,
  useServerStorage: () => h.storage,
  useServerOutbox: () => ({}),
  useServerUploads: () => ({}),
}));
vi.mock("../agents/state.ts", async (original) => ({
  ...(await original<typeof import("../agents/state.ts")>()),
  useWorkspace: () => ({ stateDir: "questions" }),
}));
vi.mock("../../conversation/historySession", () => ({
  useConversationHistory: () => ({
    items: h.items,
    loaded: true,
    before: null,
    after: null,
  }),
}));
vi.mock("../../usePendingCommands", () => ({
  usePendingCommands: () => h.empty,
}));
vi.mock("../../conversation/liveText", async (original) => ({
  ...(await original<typeof import("../../conversation/liveText")>()),
  useLiveText: () => h.liveText,
}));
vi.mock("../../servers/activity", async (original) => ({
  ...(await original<typeof import("../../servers/activity")>()),
  useServerActivity: () => {},
}));
vi.mock("../useMessageReceipts", () => ({
  useMessageReceipts: () => h.receipts,
}));
vi.mock("../removedMessages", () => ({
  useRemovedMessages: () => ({ restored: h.empty, hidden: h.hidden }),
}));
vi.mock("../usePromptRecall", () => ({ usePromptRecall: () => ({}) }));
vi.mock("../useConversationScroll", async (original) => ({
  ...(await original<typeof import("../useConversationScroll")>()),
  useConversationScroll: () => ({
    scroll: useRef<HTMLDivElement>(null),
    content: useRef<HTMLDivElement>(null),
    getFollow: () => true,
    subscribeFollow: () => () => {},
    setFollow() {},
    onScroll() {},
    hasRecentScrollInput: () => false,
    consumeHistoryScrollInput: () => false,
    remember() {},
    revealRequest() {},
    getAnchorId: () => null,
  }),
}));
vi.mock("../useAttachmentDrafts", () => ({
  useAttachmentDrafts: () => [{}, vi.fn()],
}));
vi.mock("../useUploadRecovery", () => ({
  useUploadRecovery: () => ({ pending: h.empty }),
}));
vi.mock("../useChatReadState", () => ({ useVisibleChatResult() {} }));
vi.mock("../useDisplayPhase", () => ({ useDisplayPhase: () => "thinking" }));
vi.mock("../agents/WorkerModelPicker", () => ({
  useWorkerModels: () => ({ models: h.empty }),
}));
vi.mock("../useMessageQueue", () => ({
  useMessageQueue: () => ({ items: h.empty }),
}));
vi.mock("../useSendingMessageRemoval", () => ({
  useSendingMessageRemoval: () => ({}),
}));
vi.mock("../../hooks/conversationData", () => ({
  useChatWait: () => undefined,
}));
vi.mock("../Usage", () => ({ default: () => null }));
vi.mock("../Dictation", () => ({ Dictation: () => null }));
vi.mock("../RealtimeVoice", () => ({ default: () => null }));
vi.mock("../prompt-composer/PromptInput", () => ({ default: () => null }));
vi.mock("../conversation/transcript/SelectionQuote", async (original) => ({
  ...(await original<
    typeof import("../conversation/transcript/SelectionQuote")
  >()),
  default: () => null,
}));

requestTestEnvironment();
beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  delete window.__studioPromptComposerRenderProbe;
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
    const agent = f.state.current.agents.get("lead")!;
    const root = createRoot(
      document.body.appendChild(document.createElement("div")),
    );
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
                { value: conversationActionsFixture() },
                createElement(Conversation, {
                  id: "lead",
                  agent,
                  detail: { id: "lead", canSend: true },
                  sending: false,
                  limits: null,
                }),
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
      window.__studioPromptComposerRenderProbe = (component) => {
        if (component === "conversation") renders++;
      };
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
      await act(async () => root.unmount());
      f.store.dispose();
    }
  },
);
