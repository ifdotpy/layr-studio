import { AgentSource, AgentStatus } from "../../generated/api.ts";
import { observableFixture } from "../observableFixture.tsx";

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
import TeamPanel from "./TeamFixture.tsx";

function renderTeam(
  deferred = false,
  workerFilter = "all",
  missingSummary = false,
) {
  const noop = () => {};
  const agents: import("../../types.ts").Agent[] = [
    { id: "lead", isLead: true, source: AgentSource["managed"] },
    {
      id: "approval",
      rootId: "lead",
      parentId: "lead",
      name: "Approval worker",
      status: AgentStatus["approval"],
      source: AgentSource["managed"],
      operationalStatus: deferred
        ? { reason: "question_deferred" }
        : { reason: "needs_input", wait: null },
    },
    {
      id: "finished",
      rootId: "lead",
      parentId: "lead",
      name: "Finished worker",
      status: AgentStatus["completed"],
      source: AgentSource["managed"],
      operationalStatus: { reason: "read_result" },
    },
  ];
  const fixture = observableFixture(
    agents.filter((agent) => !missingSummary || agent.id !== "approval"),
  );
  return renderToStaticMarkup(
    createElement(
      fixture.Provider,
      {},
      createElement(
        MantineProvider,
        {},
        createElement(TeamPanel, {
          leadId: "lead",
          workerFilter,
          setWorkerQuery: noop,
        }),
      ),
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
