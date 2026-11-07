import { ChatIndicatorKind as ChatOperationalKind } from "../../status/operationalStatus.ts";
import { AgentStatus } from "../../generated/api";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";

// Token animation requires a browser; the roster and state indicators render unchanged.
vi.mock("../TokenRate", () => ({ default: () => null }));
vi.mock("../../servers/ServerServicesContext", async (importOriginal) => ({
  ...(await importOriginal<
    typeof import("../../servers/ServerServicesContext")
  >()),
  useServerFileDialogs: () => ({
    canRevealPath: () => false,
    revealPath: vi.fn(),
  }),
}));
import TeamPanel from "./TeamPanel";
import type { ChatIndicator } from "../chat-status/chatStatusModel";

function renderTeam(
  deferred = false,
  workerFilter = "all",
  missingSummary = false,
) {
  const noop = () => {};
  return renderToStaticMarkup(
    createElement(
      MantineProvider,
      {},
      createElement(TeamPanel, {
        workers: [
          {
            id: "approval",
            name: "Approval worker",
            status: AgentStatus.approval,
          },
          {
            id: "finished",
            name: "Finished worker",
            status: AgentStatus.completed,
          },
        ],
        accounts: [],
        opened: null,
        answerIds: new Set<string>(),
        deferredIds: new Set(deferred ? ["approval"] : []),
        indicators: new Map<string, ChatIndicator>([
          [
            "approval",
            missingSummary
              ? { kind: "loading", label: "" }
              : deferred
                ? { kind: "paused", label: "Question deferred" }
                : { kind: "answer", label: "Needs your answer" },
          ],
          ["finished", { kind: ChatOperationalKind["none"], label: "Read" }],
        ]),
        workerQuery: "",
        setWorkerQuery: noop,
        workerFilter,
        setWorkerFilter: noop,
        completedOpen: false,
        setCompletedOpen: noop,
        setTeamOpen: noop,
        setWideTeamOpen: noop,
        saveClosed: noop,
        open: noop,
        previewResult: noop,
        remove: noop,
      }),
    ),
  );
}

it("keeps approval workers in Needs you even before a pending request reaches the renderer", () => {
  const html = renderTeam();
  expect(html).toContain('aria-label="Needs you"');
  expect(html).toContain('data-chat-status="answer"');
  expect(html).toContain("Approval worker");
  expect(html).not.toContain("Finished worker");
  expect(renderTeam(false, "answer")).toContain("Approval worker");
});

it("keeps deferred approval out of Needs you and preserves finished filtering", () => {
  expect(renderTeam(true)).not.toContain('aria-label="Needs you"');
  expect(renderTeam(true, "answer")).not.toContain("Approval worker");
  expect(renderTeam(false, "completed")).not.toContain("Approval worker");
});

it("does not infer a status claim from raw approval before its operational summary", () => {
  expect(renderTeam(false, "all", true)).not.toContain(
    'data-chat-status="answer"',
  );
});
