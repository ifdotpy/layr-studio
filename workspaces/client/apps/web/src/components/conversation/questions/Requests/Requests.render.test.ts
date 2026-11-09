import { createRequestsView } from "../../support/requestView.ts";
// @vitest-environment happy-dom
import { act, createElement, useMemo } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createSubscriptions } from "../../../../sync/subscriptions/module.ts";
import { createServerStore } from "../../../../store/server.ts";
import { agentSummaryFixture } from "../../../../servers/agentSummaryFixture.ts";
import { approval, question } from "../../../../approvalFixtures.ts";
import type { Approval } from "../../../../approvals.ts";
import Requests from "./Requests.tsx";
import {
  requestTestEnvironment,
  requestTestRoot,
} from "./requestTestFixture.ts";
import { createQuestionCards } from "../../support/useQuestionCards.ts";
import { observer } from "mobx-react-lite";
import { requestRenderProbe } from "../RequestArea/RequestArea.tsx";
import TurnHistory from "../../transcript/TurnHistory/TurnHistory.fixture.tsx";

const h = vi.hoisted(() => ({
  store: null as ReturnType<typeof createServerStore> | null,
  answer: vi.fn(async () => ({ kind: "answer", result: {} })),
  post: vi.fn(async () => ({})),
  storage: new Storage(),
}));
vi.mock(
  "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerStore: () => h.store,
    useServerApi: () => ({ post: h.post, command: { answer: h.answer } }),
    useServerServices: () => null,
    useServerStorage: () => h.storage,
  }),
);
vi.mock("../../../../unsent/useUnsentText.ts", () => ({
  useUnsentText: () => [null, vi.fn(), vi.fn()],
}));
requestTestEnvironment();
beforeEach(() => {
  vi.useFakeTimers();
  h.answer.mockClear();
  h.post.mockClear();
});
afterEach(() => {
  vi.useRealTimers();
  delete window.__studioRequestRenderProbe;
});
const request = (id = "question") =>
  approval({
    id,
    agent: "lead",
    request: {
      kind: "agent_question",
      questions: [question("choice", "Continue?")],
    },
    params: { itemId: "item" },
  });
async function fixture(onLoading?: () => void) {
  const subscriptions = createSubscriptions({
    watchResourceChanges: () => () => {},
    watchResourceConnection: (callback) => {
      callback("live", "open", {});
      return () => {};
    },
  });
  const store = createServerStore(subscriptions);
  h.store = store;
  const root = requestTestRoot(() => {
    store.dispose();
    subscriptions.dispose();
  });
  const item = { id: "lead:item", role: "assistant", text: "Continue?" };
  const items = [item];
  const Answer = observer(function Answer({
    select,
  }: {
    select: ReturnType<typeof createQuestionCards>;
  }) {
    return createElement(
      "output",
      { "data-history": true },
      select(item)?.questions[0]?.answers?.join(",") || "waiting",
    );
  });
  const History = observer(function QuestionHistoryHarness() {
    requestRenderProbe("question-history");
    const select = useMemo(
      () =>
        createQuestionCards(
          h.store!,
          createRequestsView(h.store!, { agentId: "lead", includeTeam: true }),
          "lead",
        ),
      [h.store],
    );
    return createElement(
      "div",
      null,
      createElement(Answer, { select }),
      createElement(TurnHistory, {
        items,
        enabled: false,
        storageKey: "test",
        questionCards: select,
        renderMessage: (item) => createElement("p", null, item.text),
        onJump() {},
      }),
    );
  });
  await root.render(
    createElement(
      "div",
      null,
      createElement(Requests, { agentId: "lead", includeTeam: true }),
      createElement(History),
    ),
    { scope: "test", notify() {} },
  );
  await act(async () => vi.advanceTimersByTimeAsync(100));
  const publish = async (
    requests = [request(), request("other")],
    name = "Owner",
    history: Approval[] = [],
    owners: ReturnType<typeof agentSummaryFixture>[] = [],
  ) =>
    act(async () => {
      subscriptions.publish({ kind: "state" }, [
        {
          collection: "agentSummary",
          id: "lead",
          value: agentSummaryFixture("lead", {
            name,
            rootId: "lead",
            threadId: "thread",
          }),
        },
        ...owners.map((value) => ({
          collection: "agentSummary" as const,
          id: value.id,
          value,
        })),
        ...requests.map((value) => ({
          collection: "request" as const,
          id: value.id,
          value,
        })),
        ...history.map((value) => ({
          collection: "questionHistory" as const,
          id: value.id,
          value,
        })),
      ]);
      await vi.advanceTimersByTimeAsync(0);
    });
  onLoading?.();
  await publish();
  await act(async () => vi.advanceTimersByTimeAsync(100));
  await act(async () => {
    subscriptions.publish(
      { kind: "agentDetail", agentId: "lead" },
      { id: "lead" },
    );
    await vi.advanceTimersByTimeAsync(0);
  });
  return {
    publish,
    close: root.close,
  };
}
it("isolates an owner rename from membership, descriptions, controls and the other request", async () => {
  const f = await fixture();
  const renders = new Map<string, number>();
  window.__studioRequestRenderProbe = (leaf, id) => {
    const key = leaf + ":" + (id || "");
    renders.set(key, (renders.get(key) || 0) + 1);
  };
  try {
    await f.publish(undefined, "Renamed owner");
    expect(document.querySelector(".request-heading strong")?.textContent).toBe(
      "Renamed owner",
    );
    expect(renders.get("owner:question")).toBe(1);
    for (const leaf of [
      "list:",
      "card:question",
      "description:question",
      "controls:question",
      "status:question",
    ])
      expect(renders.get(leaf) || 0, leaf).toBe(0);
  } finally {
    await f.close();
  }
});
it("updates request state without rebuilding descriptions, owner names or list membership", async () => {
  const f = await fixture();
  const renders = new Map<string, number>();
  window.__studioRequestRenderProbe = (leaf, id) => {
    const key = leaf + ":" + (id || "");
    renders.set(key, (renders.get(key) || 0) + 1);
  };
  try {
    await f.publish([
      { ...request(), state: { kind: "closed", reason: "expired", at: 2 } },
      request("other"),
    ]);
    expect(document.querySelector(".request-status")?.textContent).toBe(
      "Closed",
    );
    for (const leaf of [
      "list:",
      "card:question",
      "description:question",
      "owner:question",
      "card:other",
      "status:other",
    ])
      expect(renders.get(leaf) || 0, leaf).toBe(0);
  } finally {
    await f.close();
  }
});
it("reads answered history and pending membership from generated projections without parent snapshots", async () => {
  const f = await fixture();
  try {
    await f.publish([request()], "Owner", [
      {
        ...request(),
        state: {
          kind: "answered",
          answer: {
            kind: "answers",
            answers: [{ id: "choice", answer: ["Yes"] }],
          },
          by: { kind: "owner" },
          at: 2,
        },
      },
    ]);
    expect(document.querySelector("[data-history]")?.textContent).toBe("Yes");
    expect(document.querySelectorAll("[data-request]")).toHaveLength(1);
    await f.publish([]);
    expect(document.querySelectorAll("[data-request]")).toHaveLength(0);
    expect(document.querySelector("[data-history]")?.textContent).toBe(
      "waiting",
    );
  } finally {
    await f.close();
  }
});
it("retains local answer drafts until the request projection confirms membership", async () => {
  const { requestDraftsForStorage } =
    await import("../support/requestDrafts.ts");
  const { answerKey } = await import("../RequestArea/RequestArea.tsx");
  const drafts = requestDraftsForStorage(h.storage);
  const key = answerKey("test", request());
  drafts.set(key, { choice: "Keep my answer" });
  const f = await fixture(() => expect(drafts.has(key)).toBe(true));
  try {
    expect(drafts.has(key)).toBe(true);
    await f.publish([]);
    expect(drafts.has(key)).toBe(false);
  } finally {
    await f.close();
  }
});

for (const changed of ["question", "options", "owner"] as const) {
  it(`resets only the answer editor when its full identity changes (${changed})`, async () => {
    const f = await fixture();
    try {
      const card = document.querySelector('[data-request="question"]')!;
      await act(async () =>
        card.querySelector<HTMLButtonElement>("[data-answer]")!.click(),
      );
      const field = card.querySelector<HTMLTextAreaElement>(
        'textarea[aria-label="Continue?"]',
      )!;
      await act(async () => {
        Object.getOwnPropertyDescriptor(
          HTMLTextAreaElement.prototype,
          "value",
        )!.set!.call(field, "Stale answer");
        field.dispatchEvent(new Event("input", { bubbles: true }));
      });
      expect(field.value).toBe("Stale answer");
      const updated = request();
      if (changed === "owner") updated.agent = "worker";
      else
        updated.request = {
          kind: "agent_question",
          questions: [
            question(
              "choice",
              changed === "question" ? "Changed question?" : "Continue?",
              changed === "options" ? ["New option"] : [],
            ),
          ],
        };
      const renders = new Map<string, number>();
      window.__studioRequestRenderProbe = (leaf, id) => {
        const key = leaf + ":" + (id || "");
        renders.set(key, (renders.get(key) || 0) + 1);
      };
      await f.publish(
        [updated, request("other")],
        "Owner",
        [],
        [agentSummaryFixture("worker", { rootId: "lead" })],
      );
      expect(document.querySelector('[data-request="question"]')).toBe(card);
      expect(
        card.querySelector<HTMLTextAreaElement>(
          'textarea:not([aria-label="autosize measurement"])',
        )?.value || "",
      ).toBe("");
      expect(renders.get("card:question") || 0).toBe(0);
      expect(renders.get("card:other") || 0).toBe(0);
      await act(async () =>
        card
          .querySelector("form")!
          .dispatchEvent(
            new Event("submit", { bubbles: true, cancelable: true }),
          ),
      );
      expect(h.answer).not.toHaveBeenCalled();
    } finally {
      await f.close();
    }
  });
}
it("isolates another chat's existing request state and observes this chat's history in its row", async () => {
  const f = await fixture();
  const outside = { ...request("outside"), agent: "outside" };
  const owner = agentSummaryFixture("outside", { rootId: "outside" });
  const renders = new Map<string, number>();
  try {
    await f.publish([request(), outside], "Owner", [], [owner]);
    window.__studioRequestRenderProbe = (leaf) =>
      renders.set(leaf, (renders.get(leaf) || 0) + 1);
    await f.publish(
      [
        request(),
        { ...outside, state: { kind: "closed", reason: "expired", at: 2 } },
      ],
      "Owner",
      [],
      [owner],
    );
    expect(renders.get("question-history") || 0).toBe(0);
    renders.clear();
    await f.publish(
      [request(), outside],
      "Owner",
      [
        {
          ...request(),
          state: {
            kind: "answered",
            answer: {
              kind: "answers",
              answers: [{ id: "choice", answer: ["Yes"] }],
            },
            by: { kind: "owner" },
            at: 2,
          },
        },
      ],
      [owner],
    );
    expect(document.querySelector("[data-history]")?.textContent).toBe("Yes");
    expect(document.querySelector(".question-card-state")?.textContent).toBe(
      "Answered",
    );
    expect(renders.get("question-history") || 0).toBe(0);
  } finally {
    await f.close();
  }
});
