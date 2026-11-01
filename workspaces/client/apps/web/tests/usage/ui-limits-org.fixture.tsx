import type { Agent } from "../../src/types.ts";
import type { Account } from "../../src/components/Accounts.tsx";
import type { components } from "../../src/generated/api.ts";
import { projectedLimits } from "../../src/usage/accountLimits.fixture.ts";
import { fixtureRoot } from "./usage-fixture.ts";

import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider, Button } from "@mantine/core";
import "@mantine/core/styles.css";
import Usage, { type UsageAccount } from "../../src/components/Usage.tsx";
import { NativeError } from "../../src/components/NativeNotice.tsx";
import Accounts from "../../src/components/Accounts.tsx";
import ClaudeProfile from "../../src/components/ClaudeProfile.tsx";
import AccountSignInNotice from "../../src/components/AccountSignInNotice.tsx";
import "../../src/components/accounts.css";
import "../../src/style.css";
import "../../src/appearance.css";
import { theme } from "../../src/theme.ts";
declare global {
  interface Window {
    refreshFixture: (mode?: "rejected" | "allowed" | "in_use") => void;
    signedInAccount: string | undefined;
  }
}
const expected = {
  name: "Research organization for the selected Claude profile",
  idSuffix: "12345678",
};
const observed = {
  name: "Another organization with a separate subscription",
  idSuffix: "87654321",
};
// The base one-off account omits its home and source.
function partialOrganizationAccount(
  value: Omit<Account, "home" | "source">,
): Account {
  return value as Account;
}
const account = partialOrganizationAccount({
  id: "claude",
  label: "Research",
  email: "research@example.test",
  provider: "claude",
  status: "changed",
  organization: expected,
  organizationMismatch: { expected, observed },
});
const now = Date.now() / 1000;
const agent: Agent = {
  id: "worker",
  rootId: "worker",
  epoch: 1,
  isLead: true,
  status: "failed",
  accountKey: "claude",
  provider: "claude",
  model: "sonnet",
  threadId: "thread",
  nativeLimitErrorAt: now - 20,
  error: {
    codexErrorInfo: "rateLimitExceeded",
    message: "Usage limit reached",
  },
};
function Fixture() {
  const [screen, setScreen] = useState("limits");
  const [mode, setMode] = useState<"rejected" | "allowed" | "in_use">(
    "rejected",
  );
  const [limitsOpen, setLimitsOpen] = useState(false);
  window.refreshFixture = (next = mode) => setMode(next);
  const view: components["schemas"]["RateLimitView"] = projectedLimits(
    "claude",
    now,
  );
  view.observation.data = {
    rateLimits: {
      limitId: "claude",
      primary: { usedPercent: 100, resetsAt: now + 3600 },
    },
  };
  view.projection.agents.worker = {
    eligibility: {
      agentId: "worker",
      accountKey: "claude",
      provider: "claude",
      model: "sonnet",
      state: mode === "rejected" ? "blocked" : "allowed",
      validUntil: now + 60,
      buckets: [
        { id: "claude", overageAllowed: mode !== "rejected" },
        { id: "claude-sonnet", overageAllowed: mode !== "rejected" },
      ],
      overage: mode,
    },
    recovery:
      mode === "rejected"
        ? {
            title: "Limit reached",
            message: "Limit reached",
            timing: { kind: "waiting", resetAt: now + 3600 },
            action: { kind: "none" },
          }
        : null,
  };
  view.projection.buckets.current.buckets = [
    {
      id: "claude",
      name: "Claude",
      shortName: "Claude",
      planType: null,
      creditsLabel: null,
      windows: [
        {
          label: "Session",
          color: "paused",
          expired: false,
          remaining: 0,
          reset: now + 3600,
          warning: true,
        },
      ],
    },
  ];
  const limits = view.observation;
  const recovery = view.projection.agents.worker.recovery;
  return (
    <MantineProvider theme={theme} defaultColorScheme="auto">
      <main style={{ padding: 12, maxWidth: 900, margin: "auto", minWidth: 0 }}>
        <Button
          onClick={() => setScreen(screen === "limits" ? "accounts" : "limits")}
        >
          {screen === "limits" ? "Show accounts" : "Show limits"}
        </Button>
        {screen === "limits" ? (
          <>
            <div data-notice>
              {recovery && (
                <NativeError
                  agent={agent}
                  recovery={recovery}
                  refreshLimits={() => window.refreshFixture()}
                />
              )}
            </div>
            <div
              style={{ position: "fixed", bottom: 12, right: 12 }}
              data-usage
            >
              <Usage
                agent={agent}
                stateDir="fixture"
                limits={limits}
                accounts={[
                  partialOrganizationUsageAccount({
                    key: "claude",
                    provider: "claude",
                    label: "Research",
                    limits,
                    ...view.projection,
                    reload: async () => window.refreshFixture(),
                  }),
                ]}
                accountLabel="Research"
                opened={limitsOpen}
                onChange={setLimitsOpen}
                reload={async () => window.refreshFixture()}
              />
            </div>
          </>
        ) : (
          <>
            <AccountSignInNotice
              account={account}
              errors={[]}
              onSignIn={(key) => (window.signedInAccount = key)}
            />
            <Accounts
              managerOnly
              state={partialOrganizationState({
                data: {
                  accounts: [account],
                  archivedAccounts: [],
                  defaultAccountKey: "claude",
                  defaultAccountGroups: [],
                  logins: [],
                },
                setData: () => {},
                refresh: async () => {},
                error: "",
                scope: "fixture",
              } satisfies PartialOrganizationState)}
              onError={() => {}}
            />
            <ClaudeProfile account={account} onSaved={() => {}} expanded />
          </>
        )}
      </main>
    </MantineProvider>
  );
}
createRoot(fixtureRoot()).render(<Fixture />);

// The base one-off usage account omits loading.
function partialOrganizationUsageAccount(
  value: Omit<UsageAccount, "loading">,
): UsageAccount {
  return value as UsageAccount;
}
type OrganizationState = React.ComponentProps<typeof Accounts>["state"];
type PartialOrganizationState = Omit<OrganizationState, "data" | "refresh"> & {
  data: Omit<
    NonNullable<OrganizationState["data"]>,
    "supportsDelete" | "supportsDisconnect"
  >;
  refresh: (...args: Parameters<OrganizationState["refresh"]>) => Promise<void>;
};
// Base state omits supportsDelete/supportsDisconnect; refresh resolves undefined.
function partialOrganizationState(value: unknown): OrganizationState {
  return value as OrganizationState;
}
