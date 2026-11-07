import { AgentSource } from "../generated/api.ts";
import { agentSummaryFixture } from "../servers/agentSummaryFixture.ts";
import { describe, expect, it } from "vitest";
import {
  formatOperationalStatus,
  formatAgentStatus,
  agentIndicators,
} from "./operationalStatus.ts";

describe("operational status presentation", () => {
  it("makes no status claim before the operational summary arrives", () => {
    expect(formatOperationalStatus(undefined)).toEqual({
      kind: "loading",
      label: "",
    });
  });
  it("formats domain reason codes without recomputing precedence", () => {
    const examples = [
      [
        "waiting_background_command",
        "working",
        "Waiting for background command",
      ],
      ["working", "working", "Working"],
      ["turn_ended", "none", "Turn ended. Send a message to continue."],
      ["disk_space", "paused", "Waiting for disk space"],
      ["failed", "error", "Failed"],
      ["interrupted", "paused", "Interrupted"],
      ["stopped", "paused", "Stopped"],
      ["question_deferred", "paused", "Question deferred"],
      ["unread_result", "unread", "Unread result"],
      ["read_result", "none", "Read"],
      ["ready", "none", "Ready"],
    ] as const;
    for (const [reason, kind, label] of examples)
      expect(formatOperationalStatus({ reason })).toEqual({ kind, label });
    expect(
      formatOperationalStatus({ reason: "needs_input", wait: null }),
    ).toEqual({ kind: "answer", label: "Needs your answer" });
  });

  it("formats wait counts and event identities once for both consumers", () => {
    const wait = {
      agents: 1,
      commands: 2,
      monitors: 1,
      inputs: 3,
      event: "build, verify",
    };
    const label =
      "Waiting for 1 agent, 2 commands, 1 monitor, event build, verify and 3 inputs";
    expect(formatOperationalStatus({ reason: "waiting", wait })).toEqual({
      kind: "working",
      label,
    });
    expect(formatOperationalStatus({ reason: "needs_input", wait })).toEqual({
      kind: "answer",
      label,
    });
    expect(
      formatOperationalStatus({
        reason: "waiting",
        wait: { agents: 0, commands: 0, monitors: 0, inputs: 1, event: null },
      }),
    ).toEqual({ kind: "working", label: "Waiting for 1 input" });
  });
});

it("sidebar and conversation trust the same supplied reason, not raw status flags", () => {
  const agent = agentSummaryFixture("chat", {
    status: "failed",
    hasApproval: true,
    operationalStatus: { reason: "working" },
  });
  expect(agentIndicators([agent]).get("chat")).toEqual(
    formatAgentStatus(agent),
  );
  expect(formatAgentStatus(agent)).toEqual({
    kind: "working",
    label: "Working",
  });
  expect(
    agentIndicators([{ id: "missing", source: AgentSource.managed }]).get(
      "missing",
    ),
  ).toEqual({ kind: "loading", label: "" });
});
it("the pending read overlay applies only to the captured completed-result proof", () => {
  const target = {
    threadId: "thread",
    turnId: "turn",
    revision: 3,
    read: false,
  };
  const agent = agentSummaryFixture("chat", {
    operationalStatus: { reason: "unread_result" },
    markRead: target,
  });
  expect(formatAgentStatus(agent, { ...target })).toEqual(
    formatOperationalStatus({ reason: "read_result" }),
  );
  expect(formatAgentStatus(agent, { ...target, revision: 2 }).kind).toBe(
    "unread",
  );
  expect(
    formatAgentStatus(
      { ...agent, markRead: { ...target, turnId: "next" } },
      target,
    ).kind,
  ).toBe("unread");
});
