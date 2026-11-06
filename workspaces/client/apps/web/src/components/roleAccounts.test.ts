import { describe, expect, it } from "vitest";
import { AgentMode } from "../generated/api";
import type { Agent } from "../types";
import type { UsageAccount } from "./Usage";
import { roleAccounts } from "./roleAccounts";

const account = (
  key: string,
  provider: "codex" | "claude",
  color: "green" | "yellow" | "red" | "gray",
  days: number | null = 3,
): UsageAccount => {
  const state = { color, days, unlimited: false, windows: [] };
  return {
    key,
    label: key,
    provider,
    limits: null,
    loading: false,
    reload: () => {},
    runway: { computedAt: 0, current: state, next: state, validUntil: null },
  };
};
const accounts = [
  account("default", "codex", "yellow"),
  account("codex-2", "codex", "green", 5),
  account("codex-3", "codex", "green", 2),
  account("claude-1", "claude", "green", 6),
  account("codex-4", "codex", "red"),
  account("codex-5", "codex", "gray"),
  account("claude-2", "claude", "yellow"),
];
const lead = (fields: Partial<Agent> = {}) =>
  ({
    id: "lead",
    accountKey: "default",
    subagentConcurrencyVersion: 2,
    concurrency: 3,
    agentModeRevision: 1,
    workerDefaults: { accountKey: null, model: "gpt-6-luna" },
    ...fields,
  }) as Agent;
const summary = (agent: Agent) =>
  roleAccounts(agent, accounts).map(
    ({ role, account, automatic }) =>
      `${role}:${account.key}${automatic ? ":auto" : ""}`,
  );

describe("role accounts", () => {
  it("shows at most one account per role, not one per account", () => {
    expect(summary(lead())).toEqual([
      "Lead:default",
      "Workers:codex-2:auto",
      "Review:codex-2:auto",
    ]);
  });

  it("shows only the lead with subagents off", () => {
    expect(summary(lead({ concurrency: 0 }))).toEqual(["Lead:default"]);
    expect(
      summary(
        lead({
          subagentConcurrencyVersion: undefined,
          agentModeSupported: true,
          agentMode: AgentMode.single,
        } as Partial<Agent>),
      ),
    ).toEqual(["Lead:default"]);
  });

  it("uses chosen worker and review accounts", () => {
    expect(
      summary(
        lead({
          workerDefaults: { accountKey: "codex-4", model: "gpt-6-luna" },
          reviewDefaults: { accountKey: "claude-2", model: null },
        } as Partial<Agent>),
      ),
    ).toEqual(["Lead:default", "Workers:codex-4", "Review:claude-2"]);
  });

  it("chooses an automatic account of the workers' model provider", () => {
    expect(
      summary(
        lead({
          workerDefaults: { accountKey: null, model: "claude-opus-5-5" },
        } as Partial<Agent>),
      ),
    ).toEqual([
      "Lead:default",
      "Workers:claude-1:auto",
      "Review:claude-1:auto",
    ]);
  });
});
