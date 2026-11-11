import "../../src/rendererStyles.ts";
import { observableFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import type { Agent } from "../../src/types.ts";
import {
  fixtureRoot,
  usageSessionCost,
  usageAccountCost,
} from "./usage-fixture.ts";

import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import type { UsageAccount } from "../../src/app/settings/UsageFixture.tsx";
import type { ServerServices } from "../../src/servers/serverServices.ts";
import {
  createSessionCostMirror,
  createCostsMirror,
} from "../../src/generated/observableResources.ts";
import { ServerStoreProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import {
  createUsageFixture,
  UsageFixture as Usage,
} from "../../src/app/settings/UsageFixture.tsx";
import { ServerServicesProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import * as sync from "../../src/sync/client.ts";
const services = partialUsageServices({
  owner: { id: "fixture", origin: location.origin, credentialId: null },
  sync: {
    ...sync,
  },
});
// Base usage services omit unrelated capabilities and the owner label.
function partialUsageServices(value: unknown): ServerServices {
  return value as ServerServices;
}
import { accountLimitFixtures } from "../../src/components/accounts/support/AccountLimits.fixtures.ts";
import { theme } from "../../src/theme.ts";
const now = Number(new URLSearchParams(location.search).get("now"));
const geometry = location.search.includes("geometry");
if (geometry) {
  await import("../../src/style.css");
  await import("../../src/design/state.css");
}
const own: Agent = {
  id: "chat",
  rootId: "chat",
  accountKey: "own",
  source: "managed",
  model: "gpt-6-astra",
  status: "completed",
  isLead: true,
  created: 1,
  contextUsage: { tokens: 10, window: 100 },
};
const ownLimits = {
  accountKey: "own",
  at: now,
  data: {
    accountId: "own-account",
    rateLimits: {
      limitId: "codex",
      primary: { usedPercent: 20, resetsAt: now + 3600 },
    },
  },
};
const teamLimits = {
  accountKey: "team",
  at: now,
  data: {
    accountId: "team-account",
    rateLimits: {
      limitId: "codex",
      primary: { usedPercent: 92, resetsAt: now + 3600 },
    },
    rateLimitResetCredits: {
      availableCount: 1,
      credits: [
        {
          id: "team-credit",
          title: "Team reset",
          status: "available",
          resetType: "codexRateLimits",
        },
      ],
    },
  },
};
declare global {
  interface Window {
    heartbeats: number;
    reloadCalls: [string, boolean][];
  }
}
window.heartbeats = 0;
window.reloadCalls = [];
const NativeEventSource = window.EventSource;
window.EventSource = class extends NativeEventSource {
  constructor(...args: ConstructorParameters<typeof NativeEventSource>) {
    super(...args);
    this.addEventListener("heartbeat", () => window.heartbeats++);
  }
};
window.reloadCalls = [];
const scheme = new URLSearchParams(location.search).get("scheme");
if (scheme !== null && scheme !== "light" && scheme !== "dark")
  throw new Error("Invalid fixture color scheme");

import * as api from "../../src/servers/net/api.ts";
const multi = location.search.includes("multi");
let teamReady = false;
const setTeamReady = (value: boolean) => {
  teamReady = value;
};
const accounts: UsageAccount[] = geometry
  ? accountLimitFixtures
  : [
      {
        key: "own",
        provider: "codex",
        label: "Own account",
        email: "own@example.test",
        accountId: "own-account",
        limits: ownLimits,
        loading: false,
        reload: (force = false) => {
          window.reloadCalls.push(["own", force]);
        },
      },
      ...(multi
        ? ([
            {
              key: "team",
              label: "Team account",
              email: "team@example.test",
              provider: "codex",
              accountId: "team-account",
              limits: teamReady ? teamLimits : null,
              loading: false,
              reload: (force = false) => {
                window.reloadCalls.push(["team", force]);
                if (!teamReady) setTeamReady(true);
              },
            },
            {
              key: "claude",
              label: "Claude work",
              email: "claude@example.test",
              provider: "claude",
              limits: {
                accountKey: "claude",
                at: now,
                data: {
                  rateLimits: {
                    limitId: "claude",
                    primary: { usedPercent: 50, resetsAt: now + 3600 },
                  },
                },
              },
              loading: false,
              reload: (force = false) => {
                window.reloadCalls.push(["claude", force]);
              },
            },
            {
              key: "unnamed",
              label: "unnamed",
              provider: "codex",
              limits: {
                accountKey: "unnamed",
                at: now,
                data: {
                  rateLimits: {
                    limitId: "codex",
                    primary: { usedPercent: 85, resetsAt: now + 3600 },
                  },
                },
              },
              loading: false,
              reload: (force = false) => {
                window.reloadCalls.push(["unnamed", force]);
              },
            },
          ] satisfies UsageAccount[])
        : []),
    ];
const models = observableFixture([own]);
const session = createSessionCostMirror(usageSessionCost());
const costs = createCostsMirror([usageAccountCost("own")]);
const store = {
  ...models.store,
  sessionCost: () => ({
    current: { status: "live" as const, data: session.current },
  }),
  costs: () => ({
    current: { status: "live" as const, data: costs.current },
  }),
};
const usageAgent = geometry ? { ...own, accountKey: "codex-main" } : own;
const owner = createUsageFixture(store, api, { agent: usageAgent, accounts });
owner.start();
window.addEventListener(
  "pagehide",
  () => {
    owner.dispose();
    models.store.dispose();
  },
  { once: true },
);
function Fixture() {
  return (
    <MantineProvider
      theme={theme}
      {...(scheme === "light" || scheme === "dark"
        ? { forceColorScheme: scheme }
        : {})}
    >
      <ServerStoreProvider value={store}>
        <div
          style={
            geometry
              ? {
                  display: "flex",
                  alignItems: "end",
                  width: "100%",
                  height: "100dvh",
                  padding: "var(--mantine-spacing-sm)",
                  containerType: "inline-size",
                  containerName: "conversation-footer",
                }
              : undefined
          }
        >
          <Usage
            owner={owner}
            agent={geometry ? { ...own, accountKey: "codex-main" } : own}
            stateDir="/fixture"
            limits={ownLimits}
            accountLabel="own@example.test"
            reload={() => {
              window.reloadCalls.push(["fallback", true]);
            }}
            opened={true}
            onChange={() => {}}
            limitsLoading={false}
            accounts={accounts}
          />
        </div>
      </ServerStoreProvider>
    </MantineProvider>
  );
}
createRoot(fixtureRoot()).render(
  <ServerServicesProvider services={services}>
    <Fixture />
  </ServerServicesProvider>,
);
