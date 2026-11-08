import {
  QuestionDecision,
  type components,
} from "../../../../generated/api.ts";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { approval, question } from "../../../../approvalFixtures.ts";
import type { Approval } from "../../../../approvals.ts";
import { agentSummaryFixture } from "../../../../servers/agentSummaryFixture.ts";
import { requestFixture } from "../support/requestFixtures.ts";
import Requests from "./Requests.tsx";
import {
  requestTestEnvironment,
  requestTestRoot,
} from "./requestTestFixture.ts";

const h = vi.hoisted(() => ({
  store: null as ReturnType<typeof requestFixture>["store"] | null,
  storage: new Storage(),
  post: vi.fn(async () => ({})),
  answer: vi.fn(async () => ({
    kind: "answer",
    result: { status: "answered" },
  })),
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
  useUnsentText: () => [null, vi.fn(), vi.fn(async () => {})],
}));
let root: ReturnType<typeof requestTestRoot>;
const refresh = vi.fn(async () => {}),
  notify = vi.fn();
requestTestEnvironment();
beforeEach(() => {
  root = requestTestRoot(() => h.store?.dispose());
});
afterEach(async () => {
  await root.close();
  h.post.mockClear();
  h.answer.mockClear();
  refresh.mockClear();
  notify.mockClear();
});
async function render(request: Approval, threadId?: string | null) {
  h.store = requestFixture(
    [request],
    threadId === undefined ? [] : [agentSummaryFixture("lead", { threadId })],
    threadId === undefined
      ? []
      : [
          {
            id: "lead",
            error: { codexErrorInfo: "misalignmentPolicyViolation" },
          },
        ],
  ).store;
  await root.render(createElement(Requests, { all: true }), {
    scope: "server",
    refresh,
    notify,
  });
}
function button(label: string) {
  const button = [...document.querySelectorAll("button")].find(
    (b) => b.textContent?.trim() === label,
  );
  expect(button, label).toBeDefined();
  return button!;
}
const click = (label: string) => act(async () => button(label).click());
async function fill(label: string, value: string) {
  const field = document.querySelector<HTMLInputElement | HTMLTextAreaElement>(
    `[aria-label="${label}"]`,
  )!;
  expect(field).not.toBeNull();
  await act(async () => {
    const prototype =
      field.tagName === "TEXTAREA"
        ? HTMLTextAreaElement.prototype
        : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(prototype, "value")!.set!.call(
      field,
      value,
    );
    field.dispatchEvent(new Event("input", { bubbles: true }));
  });
}
it("Answer expands and hides the editor inside the same request card", async () => {
  await render(
    approval({
      request: {
        kind: "agent_question",
        questions: [question("choice", "Continue?")],
      },
    }),
  );
  const card = document.querySelector("[data-request]");
  expect(button("Answer").getAttribute("aria-expanded")).toBe("false");
  await click("Answer");
  expect(document.querySelector("[data-request]")).toBe(card);
  expect(document.querySelectorAll("[data-request]")).toHaveLength(1);
  expect(document.querySelectorAll(".request-inline-answer")).toHaveLength(1);
  await click("Hide");
  expect(document.querySelectorAll(".request-inline-answer")).toHaveLength(0);
  expect(h.post).not.toHaveBeenCalled();
});
for (const decision of ["accept", "decline"] as const) {
  it(`approval ${decision} retains its card and sends the exact request once`, async () => {
    await render(
      approval({
        id: "approval",
        request: { kind: "exec", command: "pwd", cwd: "/", reason: null },
      }),
    );
    const card = document.querySelector("[data-request]");
    await act(async () => {
      const action = button(decision === "accept" ? "Approve" : "Decline");
      action.click();
      action.click();
    });
    expect(document.querySelector("[data-request]")).toBe(card);
    expect(document.querySelectorAll("[data-request]")).toHaveLength(1);
    expect(h.answer).toHaveBeenCalledTimes(1);
    expect(h.answer).toHaveBeenCalledWith("approval", { decision });
    expect(refresh).toHaveBeenCalledTimes(1);
    expect(notify).not.toHaveBeenCalled();
  });
}
for (const threadId of [null, "thread-1"]) {
  it(`keeps the native thread refusal scoped to a real matching thread (${threadId})`, async () => {
    await render(
      approval({
        id: "approval",
        request: { kind: "exec", command: "pwd", cwd: "/", reason: null },
        params: { threadId },
      }),
      threadId,
    );
    const approve = [...document.querySelectorAll("button")].find(
      (button) => button.textContent === "Approve",
    );
    expect(Boolean(approve)).toBe(threadId === null);
    if (approve) {
      await act(async () => approve.click());
      expect(h.answer).toHaveBeenCalledWith("approval", { decision: "accept" });
    } else expect(h.answer).not.toHaveBeenCalled();
  });
}
for (const kind of ["agent_question", "question"] as const) {
  it(`sends only the answer data for ${kind}`, async () => {
    await render(
      approval({
        id: "q",
        request: { kind, questions: [question("q", "Choose")] },
      }),
    );
    await click("Answer");
    await fill("Choose", "Chosen");
    await act(async () =>
      document
        .querySelector("form")!
        .dispatchEvent(
          new Event("submit", { bubbles: true, cancelable: true }),
        ),
    );
    expect(h.answer).toHaveBeenCalledWith("q", {
      answers: { q: { answers: ["Chosen"] } },
    });
  });
}
it("sends elicitation content through its generated answer variant", async () => {
  await render(
    approval({
      id: "form",
      request: {
        kind: "elicitation",
        schema: {
          properties: { count: { type: "integer" }, flag: { type: "boolean" } },
          required: ["count", "flag"],
        },
      },
      params: { mode: "form" },
    }),
  );
  await click("Answer");
  await fill("count", "7");
  await fill("flag", "true");
  await act(async () =>
    document
      .querySelector("form")!
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })),
  );
  expect(h.answer).toHaveBeenCalledWith("form", {
    decision: "accept",
    content: { count: 7, flag: true },
  });
});
const answer = {
  kind: "answers" as const,
  answers: [{ id: "q", answer: ["Yes"] }],
};
const stateCases: [Approval["state"], boolean][] = [
  [{ kind: "pending" }, true],
  [{ kind: "answering", answer, command: "command" }, false],
  [{ kind: "answered", answer, by: { kind: "owner" }, at: 2 }, false],
  [{ kind: "unknown", answer, since: 2 }, false],
  [{ kind: "closed", reason: "expired", at: 2 }, false],
];
for (const [state, controls] of stateCases) {
  it(`shows answer controls only while a question is pending (${state.kind})`, async () => {
    await render(
      approval({
        request: {
          kind: "agent_question",
          questions: [question("q", "Continue?")],
        },
        state,
      }),
    );
    expect(!!document.querySelector('[data-answer="approval"]')).toBe(controls);
  });
}
const approvalCases: Approval["request"][] = [
  { kind: "patch", item: "item", files: ["a.ts"], reason: null },
  { kind: "permissions", requested: ["network"] },
  { kind: "tool_use", tool: "Bash", input: {} },
  { kind: "monitor_start", monitor: "m", command: "tail -f log" },
];
for (const request of approvalCases) {
  it(`offers approve and decline for a ${request.kind} request`, async () => {
    await render(approval({ request }));
    expect(button("Approve")).toBeDefined();
    expect(button("Decline")).toBeDefined();
  });
}
// These deliberately invalid fresh answers must fail against the same generated
// contract used by AnswerForm; opaque legacy HTTP answers keep their own boundary.
function questionAnswerConstructionWitness() {
  type Answer = components["schemas"]["QuestionCardAnswer"];
  // @ts-expect-error a question answer requires its selected-answer map
  const missingAnswers: Answer = { method: "agent/asyncQuestion", data: {} };
  // @ts-expect-error elicitation requires content alongside its decision
  const missingContent: Answer = {
    method: "mcpServer/elicitation/request",
    data: { decision: QuestionDecision.accept },
  };
  const wrongMethod: Answer = {
    method: "item/commandExecution/requestApproval",
    // @ts-expect-error an approval cannot use the question-answer payload
    data: { answers: {} },
  };
  void [missingAnswers, missingContent, wrongMethod];
}
void questionAnswerConstructionWitness;
