import type { components } from "../../src/generated/api.ts";
import { projectedLimits } from "../../src/usage/accountLimits.fixture.ts";
import type { Agent, Message } from "../../src/types.ts";
import { fixtureRoot } from "./usage-fixture.ts";

import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import {
  NativeError,
  NativeNotice,
} from "../../src/components/NativeNotice.tsx";
import Usage from "../../src/components/Usage.tsx";
const now = Number(new URLSearchParams(location.search).get("now"));
const error = {
  codexErrorInfo: "usageLimitExceeded",
  message: "Historical quota failure",
};
const agent: Agent = {
  id: "lumina",
  rootId: "lumina",
  accountKey: "work",
  threadId: "native-lumina",
  model: "gpt-6-astra",
  provider: "codex",
  error,
  nativeLimitErrorAt: now - 10,
  status: "failed",
  isLead: true,
};
const item: Message = {
  id: "past-error",
  role: "system",
  nativeNotice: "error",
  nativeError: error,
  text: error.message,
};
export type LimitSnapshot = {
  label: string;
  limits: components["schemas"]["RateLimitView"];
  agentId?: string;
  accountKey?: string;
  threadId?: string;
  turnId?: Agent["turnId"];
  lastCompletedTurn?: Agent["lastCompletedTurn"];
  episode?: number;
  mountKey?: number;
};
declare global {
  interface Window {
    // fixtureActions is a tuple array in tests/performance/sidebar-incremental-render.fixture.tsx.
    recoveryActions: string[];
    applyLimitSnapshot: (snapshot: LimitSnapshot) => void;
  }
}
window.recoveryActions = [];
const action = () => {
  window.recoveryActions.push("action");
};
function parseSnapshot(serialized: string): LimitSnapshot | null {
  // This fixture owns both the session document writer and reader.
  return JSON.parse(serialized) as LimitSnapshot | null;
}
function Fixture() {
  const [snapshot, setSnapshot] = useState<LimitSnapshot>(
    () =>
      parseSnapshot(sessionStorage.getItem("fixture-initial") || "null") || {
        label: "initial",
        limits: initialLimits(),
      },
  );
  window.applyLimitSnapshot = setSnapshot;
  const recovery =
    snapshot.limits.projection.agents[snapshot.agentId || agent.id]?.recovery;
  const currentAgent = {
    ...agent,
    error: recovery ? agent.error : undefined,
    id: snapshot.agentId || agent.id,
    accountKey: snapshot.accountKey || agent.accountKey,
    threadId: snapshot.threadId || agent.threadId,
    turnId: snapshot.turnId,
    lastCompletedTurn: snapshot.lastCompletedTurn,
    nativeLimitErrorAt: snapshot.episode ?? agent.nativeLimitErrorAt,
  };
  // Base one-off Usage fixture omits stateDir; preserve that partial prop surface.
  const usageProps = partialUsageProps({
    agent: currentAgent,
    limits: snapshot.limits.observation,
    accounts: [
      {
        key: currentAgent.accountKey,
        label: "Work",
        limits: snapshot.limits.observation,
        loading: false,
        reload: action,
        ...snapshot.limits.projection,
      },
    ],
    reload: action,
    opened: true,
    onChange: action,
    limitsLoading: false,
  });
  return (
    <MantineProvider>
      <main data-case={snapshot.label}>
        <section id="current">
          {currentAgent.error && (
            <NativeError
              key={snapshot.mountKey}
              agent={currentAgent}
              recovery={recovery}
              openLimits={action}
            />
          )}
        </section>
        <section id="history">
          <NativeNotice item={item} />
        </section>
        <section style={{ position: "fixed", bottom: 20, right: 20 }}>
          <Usage key={snapshot.mountKey} {...usageProps} />
        </section>
        <output id="saved-error">{JSON.stringify(agent.error)}</output>
      </main>
    </MantineProvider>
  );
}
createRoot(fixtureRoot()).render(<Fixture />);

// The one-off base fixture omits Usage.stateDir.
function partialUsageProps(value: unknown): React.ComponentProps<typeof Usage> {
  return value as React.ComponentProps<typeof Usage>;
}

function initialLimits(): components["schemas"]["RateLimitView"] {
  const view = projectedLimits("work", now);
  view.projection.agents.lumina = {
    eligibility: {
      agentId: "lumina",
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
      message: "Limit reached",
      timing: { kind: "unknown" },
      action: { kind: "none" },
    },
  };
  return view;
}
