import type { components } from "../../src/generated/api.ts";
import { projectedLimits } from "../../src/usage/accountLimits.fixture.ts";
import type { Agent } from "../../src/types.ts";
import { fixtureRoot } from "./usage-fixture.ts";

import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import { NativeError } from "../../src/components/NativeNotice.tsx";
const dueAt = Number(new URLSearchParams(location.search).get("dueAt"));
type UsageResume = NonNullable<Agent["usageResume"]>;
// Base resume omits cause/reason; authResume omits plannedAt/reason.
function partialUsageResume(value: Partial<UsageResume>): UsageResume {
  return value as UsageResume;
}
const resume = partialUsageResume({
  id: "resume-1",
  status: "scheduled",
  dueAt: dueAt,
  plannedAt: dueAt,
  updatedAt: 1,
});
const authResume = partialUsageResume({
  id: "auth-resume-1",
  status: "scheduled",
  cause: "auth",
  dueAt: dueAt,
  updatedAt: 1,
});
const agent: Agent = {
  id: "chat-1",
  rootId: "chat-1",
  accountKey: "work",
  threadId: "thread-1",
  model: "gpt-6-astra",
  error: {
    codexErrorInfo: "usageLimitExceeded",
    message: "Usage limit reached",
  },
  nativeLimitErrorAt: dueAt - 60,
  status: "failed",
  isLead: true,
  usageResume: resume,
};
const authAgent: Agent = {
  ...agent,
  id: "chat-auth",
  error: { codexErrorInfo: "unauthorized", message: "Sign in again" },
  usageResume: authResume,
};
const view: components["schemas"]["RateLimitView"] = projectedLimits(
  "work",
  dueAt - 30,
);
view.projection.agents[agent.id] = {
  eligibility: {
    agentId: agent.id,
    accountKey: "work",
    provider: "codex",
    model: "gpt-6-astra",
    state: "blocked",
    validUntil: null,
    buckets: [],
    overage: "unavailable",
  },
  recovery: {
    title: "Limit reached",
    message: "Usage limit reached",
    timing: { kind: "waiting", resetAt: dueAt },
    action: { kind: "none" },
  },
};
createRoot(fixtureRoot()).render(
  <MantineProvider>
    <>
      <NativeError
        agent={agent}
        recovery={view.projection.agents[agent.id]?.recovery}
      />
      <NativeError agent={authAgent} />
    </>
  </MantineProvider>,
);
