import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
vi.mock("../servers/ServerServicesContext", () => ({
  useServerApi: () => ({ post: vi.fn() }),
}));
import LimitRecoveryNotice from "./LimitRecoveryNotice";
vi.mock("../local-time", () => ({ localDateTime: () => "Fixture reset time" }));
import type { components } from "../generated/api";
type LimitRecovery = components["schemas"]["LimitRecovery"];

const NOW = 1_800_000_000;
const RESET = NOW + 3600;
const recovery: LimitRecovery = {
  title: "Usage limit reached",
  message: "View account limits for reset times and available credits.",
  action: { kind: "none" },
  timing: { kind: "waiting", resetAt: RESET },
};
const scheduled = {
  id: "resume-1",
  status: "scheduled",
  cause: "usage",
  plannedAt: RESET + 60,
  updatedAt: NOW - 10,
} as never;

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(NOW * 1000);
});
afterEach(() => vi.useRealTimers());

function render(props: Record<string, unknown>) {
  return renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(LimitRecoveryNotice, {
        recovery,
        onRefresh: async () => {},
        ...props,
      }),
    ),
  );
}

function hidesDetails(html: string) {
  for (const detail of [
    "Applicable buckets",
    "Overage",
    "Current eligibility is unknown",
    "Cannot run now",
    "View account limits for reset times",
    "Reported reset",
  ])
    expect(html).not.toContain(detail);
}

test("a limit shows one line with the local reset time and a Refresh button", () => {
  const html = render({});
  expect(html).toContain("Limit reached. Resets at Fixture reset time.");
  expect(html).toContain("Refresh");
  hidesDetails(html);
});

test("automatic resume can be controlled while waiting", () => {
  const html = render({ agentId: "a", usageResume: scheduled });
  expect(html).toContain("Resets at Fixture reset time");
  expect(html).toContain("Automatic resume planned for");
  expect(html).toContain(
    `<time dateTime="${new Date((RESET + 60) * 1000).toISOString()}">`,
  );
  expect(html).not.toContain(">Refresh<");
});

test("a limit without a known reset time stays one line", () => {
  const html = render({
    recovery: { ...recovery, timing: { kind: "unknown" } },
  });
  expect(html).toContain("Limit reached.");
  expect(html).not.toContain("Resets at");
});

test("reset state follows the server projection instead of the client clock", () => {
  vi.setSystemTime((RESET + 1) * 1000);
  const waiting = {
    title: "Usage limit reached",
    message: "Wait",
    action: { kind: "none" },
    timing: { kind: "waiting", resetAt: RESET },
  };
  expect(render({ recovery: waiting })).toContain(
    "Resets at Fixture reset time",
  );
  expect(
    render({
      recovery: { ...waiting, timing: { kind: "checking", resetAt: RESET } },
    }),
  ).toContain("Studio is checking the account");
  expect(
    render({
      recovery: { ...waiting, timing: { kind: "confirmed", resetAt: RESET } },
    }),
  ).toContain("The limit has reset");
});
