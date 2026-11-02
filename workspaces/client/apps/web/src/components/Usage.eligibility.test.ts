import {
  LimitColor,
  Overage,
  ProviderKind as GeneratedApiProviderKind,
} from "../generated/api.ts";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { afterEach, expect, test, vi } from "vitest";
import type { Agent } from "../types";

vi.mock("@mantine/core", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@mantine/core")>();
  return {
    ...actual,
    Popover: Object.assign(
      (props: Parameters<typeof actual.Popover>[0]) =>
        createElement(actual.Popover, { ...props, withinPortal: false }),
      { Target: actual.Popover.Target, Dropdown: actual.Popover.Dropdown },
    ),
  };
});
vi.mock("../servers/ServerServicesContext", () => ({
  useServerServices: () => null,
  useServerApi: () => ({ get: vi.fn(), post: vi.fn(), saved: () => "" }),
  useServerSync: () => ({ subscribeStateProjection: vi.fn(() => () => {}) }),
  useServerResources: () => ({
    subscribe: vi.fn(() => () => {}),
    getSnapshot: () => ({ data: undefined, status: "stale", error: null }),
  }),
  useServerResourceReads: () => vi.fn(),
}));
vi.mock("./TokenRate", () => ({ default: () => null }));
import Usage from "./Usage";

afterEach(() => vi.useRealTimers());

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
        createElement(Usage, {
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
