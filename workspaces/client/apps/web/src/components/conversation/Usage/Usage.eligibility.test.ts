import * as fixtureApi from "../../../servers/net/api.ts";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
vi.mock("../../agents/support/state.ts", async (original) => ({
  ...(await original<typeof import("../../agents/support/state.ts")>()),
  useAgentDetail: <T>(agent: T) => agent,
}));
import {
  LimitColor,
  Overage,
  ProviderKind as GeneratedApiProviderKind,
} from "../../../generated/api.ts";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { afterEach, expect, test, vi } from "vitest";
import type { Agent } from "../../../types.ts";

vi.mock("@mantine/core", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@mantine/core")>();
  return {
    ...actual,
    Popover: Object.assign(
      (props: Parameters<typeof actual.Popover>[0]) =>
        createElement(actual.Popover, { ...props, withinPortal: false }),
      {
        Target: actual.Popover.Target,
        Dropdown: actual.Popover.Dropdown,
        extend: actual.Popover.extend,
      },
    ),
  };
});
vi.mock("../TokenRate/TokenRate.tsx", () => ({ default: () => null }));
import {
  UsageFixture,
  createUsageFixture,
  type UsageFixtureInputs,
} from "../../../app/settings/UsageFixture.tsx";

const fixtureCleanups: (() => void)[] = [];
afterEach(() => {
  for (const cleanup of fixtureCleanups.splice(0)) cleanup();
  vi.useRealTimers();
});

test("account limits omit model eligibility details and retain limits and refresh", () => {
  vi.useFakeTimers();
  const now = 1_800_000_000;
  const model = "claude-opus-5-5";
  vi.setSystemTime(now * 1000);
  const agent = {
    id: "worker",
    accountKey: "claude",
    provider: GeneratedApiProviderKind.claude,
    model,
    error: { codexErrorInfo: "rateLimitExceeded" },
    nativeLimitErrorAt: now - 10,
  } as Agent;
  for (const fresh of [false, true]) {
    const html = renderToStaticMarkup(
      createElement(
        MantineProvider,
        { env: "test" },
        usageElement({
          agent,
          stateDir: "fixture",
          limits: null,
          opened: true,
          accountLabel: "Personal",
          reload: async () => {},
          accounts: [
            {
              key: "claude",
              label: "Personal",
              provider: GeneratedApiProviderKind.claude,
              loading: false,
              reload: () => {},
              buckets: {
                current: {
                  buckets: [
                    {
                      id: "claude",
                      name: "Claude",
                      shortName: "Claude",
                      planType: null,
                      creditsLabel: null,
                      windows: [
                        {
                          label: "Primary",
                          remaining: 0,
                          reset: now + 3600,
                          expired: false,
                          color: LimitColor.error,
                          warning: true,
                        },
                      ],
                    },
                  ],
                  remaining: 0,
                  color: LimitColor.error,
                  low: true,
                },
                next: {
                  buckets: [],
                  remaining: null,
                  color: LimitColor.paused,
                  low: false,
                },
                validUntil: now + 3600,
              },
              agents: {
                [agent.id]: {
                  eligibility: {
                    agentId: agent.id,
                    accountKey: "claude",
                    provider: "claude",
                    model,
                    state: fresh ? "blocked" : "stale",
                    validUntil: fresh ? now + 60 : null,
                    buckets: [{ id: "claude-fable", overageAllowed: false }],
                    overage: Overage.rejected,
                  },
                  recovery: {
                    title: "Usage limit reached",
                    message: "Wait",
                    action: { kind: "none" },
                    timing: { kind: "waiting", resetAt: now + 3600 },
                  },
                },
              },
              limits: {
                accountKey: "claude",
                at: now,
                data: {
                  rateLimits: {
                    limitId: "claude",
                    primary: { usedPercent: 100, resetsAt: now + 3600 },
                  },
                },
              },
            },
          ],
        }),
      ),
    );
    expect(html).toContain("Account limits details");
    expect(html).toContain("Personal");
    expect(html).toContain("Primary");
    expect(html).toContain("Refresh");
    expect(html).toContain("Limit reached. Resets at");
    for (const detail of [
      "Model eligibility details",
      "model-limit-eligibility",
      "Current eligibility is unknown",
      "Refresh account limits for a current decision.",
      "Applicable buckets",
      "Overage rejected",
      "Cannot run now",
    ])
      expect(html).not.toContain(detail);
  }
});

function usageElement(props: UsageFixtureInputs) {
  const models = observableFixture([props.agent]);
  const owner = createUsageFixture(models.store, fixtureApi, props);
  owner.start();
  fixtureCleanups.push(() => {
    owner.dispose();
    models.store.dispose();
  });
  return createElement(UsageFixture, { ...props, owner });
}
